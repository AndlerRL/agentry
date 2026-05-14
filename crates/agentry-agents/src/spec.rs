use agentry_core::models::{AgentSpec, InstallMethod, PromptFormat};

/// All 11 known agent specifications.
pub fn all_agent_specs() -> Vec<AgentSpec> {
    vec![
        AgentSpec {
            id: "claude-code".into(),
            name: "Claude Code".into(),
            cli_binary: "claude".into(),
            config_dir: ".claude".into(),
            prompt_filename: "CLAUDE.md".into(),
            prompt_format: PromptFormat::PlainMd,
            skills_dir_name: Some("skills".into()),
            max_size: None,
            install_methods: vec![
                InstallMethod::DirectDownload {
                    url: "https://claude.ai/install.sh".into(),
                    binary_name: "claude".into(),
                },
                InstallMethod::Brew { formula: "claude-code".into(), cask: true },
                InstallMethod::Npm { package: "@anthropic-ai/claude-code".into() },
                InstallMethod::VsCodeExtension { extension_id: "anthropic.claude-code".into() },
            ],
        },
        AgentSpec {
            id: "continue".into(),
            name: "Continue".into(),
            cli_binary: "continue".into(),
            config_dir: ".continue".into(),
            prompt_filename: "prompts".into(), // directory, not single file
            prompt_format: PromptFormat::XmlTagMd,
            skills_dir_name: None,
            max_size: None,
            install_methods: vec![
                InstallMethod::VsCodeExtension { extension_id: "Continue.continue".into() },
                InstallMethod::JetBrainsPlugin { plugin_id: "com.continue.continue".into() },
                InstallMethod::Npm { package: "@continuedev/cli".into() },
                InstallMethod::DirectDownload {
                    url: "https://raw.githubusercontent.com/continuedev/continue/main/extensions/cli/scripts/install.sh".into(),
                    binary_name: "cn".into(),
                },
            ],
        },
        AgentSpec {
            id: "gemini-cli".into(),
            name: "Gemini CLI".into(),
            cli_binary: "gemini".into(),
            config_dir: ".gemini".into(),
            prompt_filename: "GEMINI.md".into(),
            prompt_format: PromptFormat::PlainMd,
            skills_dir_name: None,
            max_size: None,
            install_methods: vec![
                InstallMethod::Npm { package: "@google/gemini-cli".into() },
                InstallMethod::Brew { formula: "gemini-cli".into(), cask: false },
            ],
        },
        AgentSpec {
            id: "codex".into(),
            name: "Codex".into(),
            cli_binary: "codex".into(),
            config_dir: ".codex".into(),
            prompt_filename: "AGENTS.md".into(),
            prompt_format: PromptFormat::PlainMd,
            skills_dir_name: None,
            max_size: Some(32768), // 32 KiB limit
            install_methods: vec![
                InstallMethod::Npm { package: "@openai/codex".into() },
                InstallMethod::Brew { formula: "codex".into(), cask: true },
                InstallMethod::DirectDownload {
                    url: "https://github.com/openai/codex/releases".into(),
                    binary_name: "codex".into(),
                },
            ],
        },
        AgentSpec {
            id: "opencode".into(),
            name: "OpenCode".into(),
            cli_binary: "opencode".into(),
            config_dir: ".opencode".into(),
            prompt_filename: "AGENTS.md".into(),
            prompt_format: PromptFormat::FrontmatterMd,
            skills_dir_name: None,
            max_size: None,
            install_methods: vec![
                InstallMethod::Npm { package: "opencode-ai".into() },
                InstallMethod::Brew { formula: "opencode".into(), cask: false },
                InstallMethod::DirectDownload {
                    url: "https://opencode.ai/install".into(),
                    binary_name: "opencode".into(),
                },
            ],
        },
        AgentSpec {
            id: "amp".into(),
            name: "Amp".into(),
            cli_binary: "amp".into(),
            config_dir: ".amp".into(),
            prompt_filename: "AGENTS.md".into(),
            prompt_format: PromptFormat::PlainMd,
            skills_dir_name: None,
            max_size: None,
            install_methods: vec![
                InstallMethod::Npm { package: "@sourcegraph/amp".into() },
                InstallMethod::DirectDownload {
                    url: "https://ampcode.com/install.sh".into(),
                    binary_name: "amp".into(),
                },
            ],
        },
        AgentSpec {
            id: "firebender".into(),
            name: "Firebender".into(),
            cli_binary: "firebender".into(),
            config_dir: ".firebender".into(),
            prompt_filename: "rules".into(), // .mdc directory
            prompt_format: PromptFormat::Mdc,
            skills_dir_name: None,
            max_size: None,
            install_methods: vec![
                InstallMethod::JetBrainsPlugin { plugin_id: "com.firebender.firebender".into() },
            ],
        },
        AgentSpec {
            id: "openclaw".into(),
            name: "OpenClaw".into(),
            cli_binary: "openclaw".into(),
            config_dir: ".openclaw".into(),
            prompt_filename: "AGENTS.md".into(),
            prompt_format: PromptFormat::PlainMd,
            skills_dir_name: None,
            max_size: None,
            install_methods: vec![
                InstallMethod::Npm { package: "openclaw".into() },
                InstallMethod::DirectDownload {
                    url: "https://openclaw.ai/install.sh".into(),
                    binary_name: "openclaw".into(),
                },
            ],
        },
        AgentSpec {
            id: "deepagents".into(),
            name: "DeepAgents".into(),
            cli_binary: "deepagents".into(),
            config_dir: ".deepagents".into(),
            prompt_filename: "AGENTS.md".into(),
            prompt_format: PromptFormat::PlainMd,
            skills_dir_name: None,
            max_size: None,
            install_methods: vec![
                InstallMethod::Pip { package: "deepagents-cli".into() },
                InstallMethod::DirectDownload {
                    url: "https://langch.in/gh-da-cli".into(),
                    binary_name: "deepagents".into(),
                },
            ],
        },
        AgentSpec {
            id: "antigravity".into(),
            name: "Antigravity".into(),
            cli_binary: "antigravity".into(),
            config_dir: ".antigravity".into(),
            prompt_filename: "SKILL.md".into(),
            prompt_format: PromptFormat::FrontmatterMd,
            skills_dir_name: None,
            max_size: None,
            install_methods: vec![
                InstallMethod::DirectDownload {
                    url: "https://antigravity.google/download".into(),
                    binary_name: "antigravity".into(),
                },
            ],
        },
        AgentSpec {
            id: "warp".into(),
            name: "Warp".into(),
            cli_binary: "warp-cli".into(),
            config_dir: ".warp".into(),
            prompt_filename: "AGENTS.md".into(),
            prompt_format: PromptFormat::FrontmatterMd,
            skills_dir_name: None,
            max_size: None,
            install_methods: vec![
                InstallMethod::Brew { formula: "warp".into(), cask: true },
                InstallMethod::AppBundle { app_name: "Warp.app".into() },
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_agent_specs_returns_11_entries() {
        let specs = all_agent_specs();
        assert_eq!(specs.len(), 11, "expected 11 agent specs");
    }

    #[test]
    fn all_agent_specs_ids_are_unique() {
        let specs = all_agent_specs();
        let mut ids: Vec<&str> = specs.iter().map(|s| s.id.as_str()).collect();
        ids.sort();
        let mut deduped = ids.clone();
        deduped.dedup();
        assert_eq!(ids, deduped, "all ids must be unique");
    }

    #[test]
    fn claude_code_spec() {
        let specs = all_agent_specs();
        let spec = specs
            .iter()
            .find(|s| s.id == "claude-code")
            .expect("claude-code spec should exist");
        assert_eq!(spec.name, "Claude Code");
        assert_eq!(spec.cli_binary, "claude");
        assert_eq!(spec.config_dir, ".claude");
        assert_eq!(spec.prompt_filename, "CLAUDE.md");
        assert_eq!(spec.prompt_format, PromptFormat::PlainMd);
        assert_eq!(spec.skills_dir_name, Some("skills".to_string()));
        assert!(spec.max_size.is_none());
    }

    #[test]
    fn continue_spec() {
        let specs = all_agent_specs();
        let spec = specs
            .iter()
            .find(|s| s.id == "continue")
            .expect("continue spec should exist");
        assert_eq!(spec.name, "Continue");
        assert_eq!(spec.cli_binary, "continue");
        assert_eq!(spec.config_dir, ".continue");
        assert_eq!(spec.prompt_filename, "prompts");
        assert_eq!(spec.prompt_format, PromptFormat::XmlTagMd);
        assert!(spec.skills_dir_name.is_none());
        assert!(spec.max_size.is_none());
    }

    #[test]
    fn gemini_cli_spec() {
        let specs = all_agent_specs();
        let spec = specs
            .iter()
            .find(|s| s.id == "gemini-cli")
            .expect("gemini-cli spec should exist");
        assert_eq!(spec.name, "Gemini CLI");
        assert_eq!(spec.cli_binary, "gemini");
        assert_eq!(spec.config_dir, ".gemini");
        assert_eq!(spec.prompt_filename, "GEMINI.md");
        assert_eq!(spec.prompt_format, PromptFormat::PlainMd);
        assert!(spec.skills_dir_name.is_none());
        assert!(spec.max_size.is_none());
    }

    #[test]
    fn codex_spec() {
        let specs = all_agent_specs();
        let spec = specs
            .iter()
            .find(|s| s.id == "codex")
            .expect("codex spec should exist");
        assert_eq!(spec.name, "Codex");
        assert_eq!(spec.cli_binary, "codex");
        assert_eq!(spec.config_dir, ".codex");
        assert_eq!(spec.prompt_filename, "AGENTS.md");
        assert_eq!(spec.prompt_format, PromptFormat::PlainMd);
        assert!(spec.skills_dir_name.is_none());
        assert_eq!(spec.max_size, Some(32768));
    }

    #[test]
    fn opencode_spec() {
        let specs = all_agent_specs();
        let spec = specs
            .iter()
            .find(|s| s.id == "opencode")
            .expect("opencode spec should exist");
        assert_eq!(spec.name, "OpenCode");
        assert_eq!(spec.cli_binary, "opencode");
        assert_eq!(spec.config_dir, ".opencode");
        assert_eq!(spec.prompt_filename, "AGENTS.md");
        assert_eq!(spec.prompt_format, PromptFormat::FrontmatterMd);
        assert!(spec.skills_dir_name.is_none());
        assert!(spec.max_size.is_none());
    }

    #[test]
    fn amp_spec() {
        let specs = all_agent_specs();
        let spec = specs
            .iter()
            .find(|s| s.id == "amp")
            .expect("amp spec should exist");
        assert_eq!(spec.name, "Amp");
        assert_eq!(spec.cli_binary, "amp");
        assert_eq!(spec.config_dir, ".amp");
        assert_eq!(spec.prompt_filename, "AGENTS.md");
        assert_eq!(spec.prompt_format, PromptFormat::PlainMd);
        assert!(spec.skills_dir_name.is_none());
        assert!(spec.max_size.is_none());
    }

    #[test]
    fn firebender_spec() {
        let specs = all_agent_specs();
        let spec = specs
            .iter()
            .find(|s| s.id == "firebender")
            .expect("firebender spec should exist");
        assert_eq!(spec.name, "Firebender");
        assert_eq!(spec.cli_binary, "firebender");
        assert_eq!(spec.config_dir, ".firebender");
        assert_eq!(spec.prompt_filename, "rules");
        assert_eq!(spec.prompt_format, PromptFormat::Mdc);
        assert!(spec.skills_dir_name.is_none());
        assert!(spec.max_size.is_none());
    }

    #[test]
    fn openclaw_spec() {
        let specs = all_agent_specs();
        let spec = specs
            .iter()
            .find(|s| s.id == "openclaw")
            .expect("openclaw spec should exist");
        assert_eq!(spec.name, "OpenClaw");
        assert_eq!(spec.cli_binary, "openclaw");
        assert_eq!(spec.config_dir, ".openclaw");
        assert_eq!(spec.prompt_filename, "AGENTS.md");
        assert_eq!(spec.prompt_format, PromptFormat::PlainMd);
        assert!(spec.skills_dir_name.is_none());
        assert!(spec.max_size.is_none());
    }

    #[test]
    fn deepagents_spec() {
        let specs = all_agent_specs();
        let spec = specs
            .iter()
            .find(|s| s.id == "deepagents")
            .expect("deepagents spec should exist");
        assert_eq!(spec.name, "DeepAgents");
        assert_eq!(spec.cli_binary, "deepagents");
        assert_eq!(spec.config_dir, ".deepagents");
        assert_eq!(spec.prompt_filename, "AGENTS.md");
        assert_eq!(spec.prompt_format, PromptFormat::PlainMd);
        assert!(spec.skills_dir_name.is_none());
        assert!(spec.max_size.is_none());
    }

    #[test]
    fn antigravity_spec() {
        let specs = all_agent_specs();
        let spec = specs
            .iter()
            .find(|s| s.id == "antigravity")
            .expect("antigravity spec should exist");
        assert_eq!(spec.name, "Antigravity");
        assert_eq!(spec.cli_binary, "antigravity");
        assert_eq!(spec.config_dir, ".antigravity");
        assert_eq!(spec.prompt_filename, "SKILL.md");
        assert_eq!(spec.prompt_format, PromptFormat::FrontmatterMd);
        assert!(spec.skills_dir_name.is_none());
        assert!(spec.max_size.is_none());
    }

    #[test]
    fn warp_spec() {
        let specs = all_agent_specs();
        let spec = specs
            .iter()
            .find(|s| s.id == "warp")
            .expect("warp spec should exist");
        assert_eq!(spec.name, "Warp");
        assert_eq!(spec.cli_binary, "warp-cli");
        assert_eq!(spec.config_dir, ".warp");
        assert_eq!(spec.prompt_filename, "AGENTS.md");
        assert_eq!(spec.prompt_format, PromptFormat::FrontmatterMd);
        assert!(spec.skills_dir_name.is_none());
        assert!(spec.max_size.is_none());
    }

    #[test]
    fn only_claude_code_has_skills_dir_name() {
        let specs = all_agent_specs();
        let with_skills: Vec<&str> = specs
            .iter()
            .filter(|s| s.skills_dir_name.is_some())
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(with_skills, vec!["claude-code"]);
    }

    #[test]
    fn only_codex_has_max_size() {
        let specs = all_agent_specs();
        let with_max_size: Vec<&str> = specs
            .iter()
            .filter(|s| s.max_size.is_some())
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(with_max_size, vec!["codex"]);
    }

    #[test]
    fn config_dirs_start_with_dot() {
        for spec in &all_agent_specs() {
            assert!(
                spec.config_dir.starts_with('.'),
                "config_dir for {} should start with '.', got '{}'",
                spec.id,
                spec.config_dir,
            );
        }
    }

    #[test]
    fn no_empty_fields() {
        for spec in &all_agent_specs() {
            assert!(!spec.id.is_empty(), "id should not be empty");
            assert!(!spec.name.is_empty(), "name should not be empty");
            assert!(
                !spec.cli_binary.is_empty(),
                "cli_binary should not be empty"
            );
            assert!(
                !spec.config_dir.is_empty(),
                "config_dir should not be empty"
            );
            assert!(
                !spec.prompt_filename.is_empty(),
                "prompt_filename should not be empty"
            );
        }
    }
}
