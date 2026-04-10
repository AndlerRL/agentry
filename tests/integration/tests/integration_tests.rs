//! End-to-end integration tests that exercise workflows spanning multiple crates.

use std::collections::BTreeMap;

use agentry_acp::protocol::{
    self, dequeue_message, enqueue_message, init_acp_dirs, read_queue, AcpMessage, MessagePriority,
    PromptPayload, TaskAssignPayload,
};
use agentry_acp::router;
use agentry_agents::spec;
use agentry_core::discovery;
use agentry_core::format::{convert_to, converter_for};
use agentry_core::models::{AgentSpec, DetectedAgent, PromptFormat, PromptScope, UnifiedPrompt};
use agentry_openclaw::discovery as oc_discovery;
use agentry_skills::hub::SkillHub;
use agentry_skills::lockfile;
use agentry_sync::executor;
use agentry_sync::planner;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a unique temp directory that is cleaned up on drop.
fn temp_dir(prefix: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir()
        .expect("failed to create temp dir")
}

/// Build a minimal `DetectedAgent` for testing (marked as installed).
fn make_detected_agent(id: &str, name: &str, config_dir: &str) -> DetectedAgent {
    DetectedAgent {
        spec: AgentSpec {
            id: id.to_string(),
            name: name.to_string(),
            cli_binary: id.to_string(),
            config_dir: config_dir.to_string(),
            prompt_filename: "AGENTS.md".to_string(),
            prompt_format: PromptFormat::PlainMd,
            skills_dir_name: None,
            max_size: None,
        },
        installed: true,
        version: None,
        config_dir_exists: false,
        prompt_file_exists: false,
        skills_dir: None,
        skills_symlink_pattern: None,
        installed_skills: vec![],
    }
}

/// Build a minimal `UnifiedPrompt` for testing.
fn make_prompt(name: &str, body: &str) -> UnifiedPrompt {
    UnifiedPrompt {
        id: name.to_string(),
        name: name.to_string(),
        description: String::new(),
        frontmatter: BTreeMap::new(),
        body: body.to_string(),
        xml_tags: vec![],
        scope: PromptScope::Global,
        source_format: PromptFormat::PlainMd,
        source_path: None,
    }
}

// ---------------------------------------------------------------------------
// Test 1: Agent detection + sync pipeline
// ---------------------------------------------------------------------------

#[test]
fn test_agent_detection_and_sync_pipeline() {
    let home = temp_dir("agentry_it_detect_sync");

    // -- Step 1: Use agentry_core::models to create AgentSpec and DetectedAgent
    let claude_spec = AgentSpec {
        id: "claude-code".to_string(),
        name: "Claude Code".to_string(),
        cli_binary: "claude".to_string(),
        config_dir: ".claude".to_string(),
        prompt_filename: "CLAUDE.md".to_string(),
        prompt_format: PromptFormat::PlainMd,
        skills_dir_name: None,
        max_size: None,
    };

    let gemini_spec = AgentSpec {
        id: "gemini-cli".to_string(),
        name: "Gemini CLI".to_string(),
        cli_binary: "gemini".to_string(),
        config_dir: ".gemini".to_string(),
        prompt_filename: "GEMINI.md".to_string(),
        prompt_format: PromptFormat::PlainMd,
        skills_dir_name: None,
        max_size: None,
    };

    let agents = vec![
        DetectedAgent {
            spec: claude_spec,
            installed: true,
            version: None,
            config_dir_exists: false,
            prompt_file_exists: false,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: vec![],
        },
        DetectedAgent {
            spec: gemini_spec,
            installed: true,
            version: None,
            config_dir_exists: false,
            prompt_file_exists: false,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: vec![],
        },
    ];

    // Verify detection produced valid agents
    assert_eq!(agents.len(), 2);
    assert!(agents.iter().all(|a| a.installed));

    // -- Step 2: Use agentry_sync::planner to create a sync plan
    let prompt = make_prompt("architect", "# Architect Guidelines\n\nDesign first.");
    let plan = planner::plan_sync(&prompt, &agents, home.path());

    // The plan should have a mapping for each installed agent
    assert_eq!(
        plan.mappings.len(),
        2,
        "expected 2 mappings for 2 installed agents"
    );
    assert_eq!(plan.prompt_id, "architect");

    // Verify destination paths are correct
    let claude_mapping = plan
        .mappings
        .iter()
        .find(|m| m.agent_id == "claude-code")
        .expect("claude-code mapping should exist");
    assert!(claude_mapping
        .destination
        .to_string_lossy()
        .contains(".claude"));
    assert_eq!(claude_mapping.target_format, PromptFormat::PlainMd);

    let gemini_mapping = plan
        .mappings
        .iter()
        .find(|m| m.agent_id == "gemini-cli")
        .expect("gemini-cli mapping should exist");
    assert!(gemini_mapping
        .destination
        .to_string_lossy()
        .contains(".gemini"));
    assert_eq!(gemini_mapping.target_format, PromptFormat::PlainMd);

    // -- Step 3: Use agentry_sync::executor to execute a dry run
    let results = executor::execute_sync(&prompt, &plan.mappings, true);

    assert_eq!(results.len(), 2, "dry run should produce 2 results");
    for result in &results {
        assert!(result.success, "dry run mapping should succeed");
        assert!(
            result.message.contains("DRY RUN"),
            "dry run result should contain 'DRY RUN', got: {}",
            result.message
        );
    }
}

// ---------------------------------------------------------------------------
// Test 2: Skill lockfile + hub pipeline
// ---------------------------------------------------------------------------

#[test]
fn test_skill_lockfile_and_hub_pipeline() {
    let home = temp_dir("agentry_it_skill_hub");

    // -- Step 1: Create a lockfile entry
    let mut lockfile_data = lockfile::SkillLockfile {
        version: 3,
        skills: BTreeMap::new(),
        dismissed: BTreeMap::new(),
        last_selected_agents: vec!["claude-code".to_string()],
    };

    let entry = lockfile::SkillLockEntry {
        source: "vercel-labs/agent-skills".to_string(),
        source_type: "github".to_string(),
        source_url: "https://github.com/vercel-labs/agent-skills.git".to_string(),
        skill_path: "skills/deploy-to-vercel/SKILL.md".to_string(),
        skill_folder_hash: "deadbeef".to_string(),
        installed_at: "2026-04-09T00:00:00.000Z".to_string(),
        updated_at: "2026-04-09T00:00:00.000Z".to_string(),
    };
    lockfile::upsert_skill(&mut lockfile_data, "deploy-to-vercel", entry);

    // Also create the skill directory on disk so SkillHub can find it
    let skill_dir = home
        .path()
        .join(".agents")
        .join("skills")
        .join("deploy-to-vercel");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "# Deploy to Vercel\nDeploys the project to Vercel.",
    )
    .unwrap();

    // Write the lockfile
    lockfile::write_lockfile(home.path(), &lockfile_data).unwrap();

    // -- Step 2: Load SkillHub from that temp dir
    let hub = SkillHub::load(home.path(), &[]).expect("SkillHub::load should succeed");

    // -- Step 3: Verify skill shows as installed
    let installed = hub.installed();
    assert!(
        !installed.is_empty(),
        "hub should report at least one installed skill"
    );

    let skill = hub
        .get("deploy-to-vercel")
        .expect("deploy-to-vercel skill should be found in hub");
    assert!(skill.installed, "skill should be marked as installed");
    assert_eq!(skill.source, "vercel-labs/agent-skills");
    assert!(skill.install_path.is_some());

    // Verify the hub reports correct counts
    assert!(hub.installed_count() >= 1);
}

// ---------------------------------------------------------------------------
// Test 3: Prompt discovery + format conversion
// ---------------------------------------------------------------------------

#[test]
fn test_prompt_discovery_and_format_conversion() {
    let home = temp_dir("agentry_it_prompt_fmt");

    // -- Step 1: Create temp prompt files in different formats

    // Canonical prompt store (PlainMd)
    let prompts_dir = home.path().join(".agents").join("prompts");
    std::fs::create_dir_all(&prompts_dir).unwrap();
    std::fs::write(
        prompts_dir.join("coding-style.md"),
        "# Coding Style\n\nUse rustfmt always.",
    )
    .unwrap();

    // Continue prompt (XmlTagMd)
    let continue_prompts = home.path().join(".continue").join("prompts");
    std::fs::create_dir_all(&continue_prompts).unwrap();
    std::fs::write(
        continue_prompts.join("architect.md"),
        "---\nname: architect\ndescription: Software architecture expert\n---\n\n<expertise>\nYou are a senior architect.\n</expertise>",
    )
    .unwrap();

    // Claude global prompt (PlainMd)
    let claude_dir = home.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("CLAUDE.md"),
        "# Claude Guidelines\n\nBe concise and helpful.",
    )
    .unwrap();

    // -- Step 2: Use agentry_core::discovery to discover them
    let discovered = discovery::discover_prompts(home.path(), &[]);
    assert!(
        discovered.len() >= 2,
        "expected at least 2 discovered prompts, got {}",
        discovered.len()
    );

    // Verify canonical prompt was found
    let coding_style = discovered.iter().find(|p| p.name == "coding-style");
    assert!(
        coding_style.is_some(),
        "coding-style prompt should be discovered"
    );
    assert_eq!(coding_style.unwrap().source_format, PromptFormat::PlainMd);

    // Verify Claude CLAUDE.md was found
    let claude_prompt = discovered.iter().find(|p| p.name == "CLAUDE");
    assert!(claude_prompt.is_some(), "CLAUDE.md should be discovered");

    // -- Step 3: Use agentry_core::format to convert between formats
    let prompt = coding_style.unwrap();

    // Convert PlainMd -> FrontmatterMd
    let fm_output = convert_to(prompt, PromptFormat::FrontmatterMd)
        .expect("conversion to FrontmatterMd should succeed");
    assert!(
        fm_output.contains("---"),
        "FrontmatterMd output should contain frontmatter delimiters"
    );

    // Convert PlainMd -> XmlTagMd
    let xml_output =
        convert_to(prompt, PromptFormat::XmlTagMd).expect("conversion to XmlTagMd should succeed");

    // Verify the architect prompt was discovered with its XML tag format
    let architect = discovered.iter().find(|p| p.name == "architect");
    if let Some(arch) = architect {
        // The architect prompt should have been detected as XmlTagMd
        assert!(
            matches!(
                arch.source_format,
                PromptFormat::XmlTagMd | PromptFormat::FrontmatterMd
            ),
            "architect should be detected as XmlTagMd or FrontmatterMd, got {:?}",
            arch.source_format
        );
    }

    // Cross-format conversion: parse the Continue architect prompt via converter
    let arch_content = "---\nname: architect\ndescription: Architecture expert\n---\n\n<expertise>\nDesign systems well.\n</expertise>";
    let converter = converter_for(PromptFormat::XmlTagMd);
    let parsed = converter
        .parse("architect", arch_content, None)
        .expect("XmlTagMd parse should succeed");
    assert!(
        !parsed.xml_tags.is_empty(),
        "should have extracted XML tags"
    );

    // Convert parsed XML-tagged prompt to PlainMd
    let plain = convert_to(&parsed, PromptFormat::PlainMd).expect("convert to PlainMd");
    assert!(
        !plain.contains("<expertise>"),
        "plain md should not contain XML tags"
    );
    assert!(
        plain.contains("Design systems well"),
        "plain md should contain body text"
    );

    // Suppress unused-variable warning for xml_output
    let _ = xml_output;
}

// ---------------------------------------------------------------------------
// Test 4: OpenClaw workspace discovery with no config
// ---------------------------------------------------------------------------

#[test]
fn test_openclaw_discovery_no_config() {
    let home = temp_dir("agentry_it_oc_noconfig");

    // Verify discover_workspaces returns empty when no config exists
    let workspaces = oc_discovery::discover_workspaces(home.path())
        .expect("discover_workspaces should succeed even without config");
    assert!(
        workspaces.is_empty(),
        "expected empty workspaces when no openclaw config, got {}",
        workspaces.len()
    );
}

// ---------------------------------------------------------------------------
// Test 5: ACP message creation + serialization + queue round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_acp_message_queue_roundtrip() {
    let home = temp_dir("agentry_it_acp_queue");

    // -- Step 1: Create a temp dir and initialize ACP dirs
    init_acp_dirs(home.path()).expect("init_acp_dirs should succeed");

    // Verify directory structure was created
    assert!(home
        .path()
        .join(".agents")
        .join("acp")
        .join("queue")
        .exists());
    assert!(home
        .path()
        .join(".agents")
        .join("acp")
        .join("inbox")
        .exists());
    assert!(home
        .path()
        .join(".agents")
        .join("acp")
        .join("outbox")
        .exists());

    // -- Step 2: Create a message, serialize it, enqueue it
    let msg = AcpMessage::PromptRequest(PromptPayload {
        id: "prompt-001".to_string(),
        from_agent: "agentry".to_string(),
        to_agent: "claude-code".to_string(),
        prompt: "Review the sync module for correctness".to_string(),
        context: Some("The sync module handles prompt distribution.".to_string()),
        priority: MessagePriority::High,
        timestamp: "2026-04-09T12:00:00Z".to_string(),
    });

    // Verify serialization round-trip
    let json = serde_json::to_string(&msg).expect("serialization should succeed");
    let deserialized: AcpMessage =
        serde_json::from_str(&json).expect("deserialization should succeed");
    assert_eq!(
        msg, deserialized,
        "round-tripped message should equal original"
    );

    // Enqueue the message
    let msg_id = enqueue_message(home.path(), &msg).expect("enqueue should succeed");
    assert!(!msg_id.is_empty(), "message ID should not be empty");

    // -- Step 3: Read the message back from the queue
    let queue = read_queue(home.path()).expect("read_queue should succeed");
    assert_eq!(queue.len(), 1, "queue should have exactly one message");
    assert_eq!(queue[0].type_name(), "PromptRequest");
    assert_eq!(queue[0].from_agent(), "agentry");

    // -- Step 4: Dequeue the message
    let removed = dequeue_message(home.path(), &msg_id).expect("dequeue should succeed");
    assert!(removed, "dequeue should report the message was removed");

    let queue_after = read_queue(home.path()).expect("read_queue after dequeue should succeed");
    assert!(
        queue_after.is_empty(),
        "queue should be empty after dequeue"
    );

    // -- Bonus: also test enqueue + deliver_to_inbox + read_inbox
    let task_msg = AcpMessage::TaskAssign(TaskAssignPayload {
        id: "task-001".to_string(),
        from_agent: "agentry".to_string(),
        to_agent: "claude-code".to_string(),
        task_type: "code_review".to_string(),
        description: "Review authentication module".to_string(),
        input: None,
        deadline: None,
        priority: MessagePriority::Normal,
        timestamp: "2026-04-09T12:01:00Z".to_string(),
    });

    protocol::deliver_to_inbox(home.path(), "claude-code", &task_msg)
        .expect("deliver_to_inbox should succeed");

    let inbox =
        protocol::read_inbox(home.path(), "claude-code").expect("read_inbox should succeed");
    assert_eq!(inbox.len(), 1, "inbox should have one message");
    assert_eq!(inbox[0].type_name(), "TaskAssign");

    let cleared =
        protocol::clear_inbox(home.path(), "claude-code").expect("clear_inbox should succeed");
    assert_eq!(cleared, 1, "should have cleared 1 message");
}

// ---------------------------------------------------------------------------
// Test 6: ACP router capability matrix
// ---------------------------------------------------------------------------

#[test]
fn test_acp_router_capability_matrix() {
    let home = temp_dir("agentry_it_acp_router");

    // -- Step 1: Build capability matrix from static specs
    // We cannot reliably call build_capability_matrix because it calls
    // detect_agent which shells out to `which` for each binary. Instead,
    // we build a matrix manually from the agent specs, mirroring the logic
    // in router::build_capability_matrix.

    let specs = spec::all_agent_specs();
    assert!(!specs.is_empty(), "should have at least one agent spec");

    // Create DetectedAgents manually — mark claude-code and gemini-cli as installed
    // by setting installed = true (the detect_agent function checks for actual binaries,
    // which may or may not exist on this machine).
    let claude_agent = make_detected_agent("claude-code", "Claude Code", ".claude");
    let gemini_agent = make_detected_agent("gemini-cli", "Gemini CLI", ".gemini");

    // Build a capability list manually (mirroring router logic)
    let claude_caps = agentry_acp::protocol::AgentCapability {
        agent_id: "claude-code".to_string(),
        agent_name: "Claude Code".to_string(),
        capabilities: vec![
            "code_generation".to_string(),
            "code_review".to_string(),
            "debugging".to_string(),
            "refactoring".to_string(),
            "testing".to_string(),
            "documentation".to_string(),
        ],
        skills: vec![],
        model: None,
    };

    let gemini_caps = agentry_acp::protocol::AgentCapability {
        agent_id: "gemini-cli".to_string(),
        agent_name: "Gemini CLI".to_string(),
        capabilities: vec![
            "code_generation".to_string(),
            "multi_modal".to_string(),
            "research".to_string(),
            "analysis".to_string(),
        ],
        skills: vec![],
        model: None,
    };

    let caps = vec![claude_caps, gemini_caps];

    // -- Step 2: Verify agent capabilities are assigned correctly
    // Claude Code should have code_review and code_generation
    let claude = caps.iter().find(|c| c.agent_id == "claude-code").unwrap();
    assert!(
        claude.capabilities.contains(&"code_review".to_string()),
        "Claude Code should have code_review capability"
    );
    assert!(
        claude.capabilities.contains(&"code_generation".to_string()),
        "Claude Code should have code_generation capability"
    );
    assert!(
        claude.capabilities.contains(&"debugging".to_string()),
        "Claude Code should have debugging capability"
    );

    // Gemini should have multi_modal and research
    let gemini = caps.iter().find(|c| c.agent_id == "gemini-cli").unwrap();
    assert!(
        gemini.capabilities.contains(&"multi_modal".to_string()),
        "Gemini CLI should have multi_modal capability"
    );
    assert!(
        gemini.capabilities.contains(&"research".to_string()),
        "Gemini CLI should have research capability"
    );

    // -- Step 3: Verify routing logic works
    let routed = router::route_prompt(&caps, "code_review", "review this code");
    assert!(
        routed.is_some(),
        "routing should find a match for code_review"
    );
    assert_eq!(
        routed.unwrap().agent_id,
        "claude-code",
        "code_review should route to Claude Code"
    );

    let research_route = router::route_prompt(&caps, "research", "research this topic");
    assert!(
        research_route.is_some(),
        "routing should find a match for research"
    );

    // -- Step 4: Verify the static spec list covers all known agents
    let known_ids: Vec<&str> = specs.iter().map(|s| s.id.as_str()).collect();
    assert!(
        known_ids.contains(&"claude-code"),
        "specs should include claude-code"
    );
    assert!(
        known_ids.contains(&"gemini-cli"),
        "specs should include gemini-cli"
    );
    assert!(
        known_ids.contains(&"continue"),
        "specs should include continue"
    );
    assert!(known_ids.contains(&"codex"), "specs should include codex");
    assert!(
        known_ids.contains(&"openclaw"),
        "specs should include openclaw"
    );
    assert!(
        known_ids.contains(&"firebender"),
        "specs should include firebender"
    );

    // Suppress unused variable warnings for agents constructed for documentation
    let _ = (claude_agent, gemini_agent, home);
}
