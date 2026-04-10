#[allow(dead_code)]
/// Editor mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Normal,
    Insert,
    Visual,
    Command,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality_same_variants() {
        assert_eq!(EditorMode::Normal, EditorMode::Normal);
        assert_eq!(EditorMode::Insert, EditorMode::Insert);
        assert_eq!(EditorMode::Visual, EditorMode::Visual);
        assert_eq!(EditorMode::Command, EditorMode::Command);
    }

    #[test]
    fn inequality_different_variants() {
        assert_ne!(EditorMode::Normal, EditorMode::Insert);
        assert_ne!(EditorMode::Normal, EditorMode::Visual);
        assert_ne!(EditorMode::Normal, EditorMode::Command);
        assert_ne!(EditorMode::Insert, EditorMode::Visual);
        assert_ne!(EditorMode::Insert, EditorMode::Command);
        assert_ne!(EditorMode::Visual, EditorMode::Command);
    }

    #[test]
    fn debug_formatting_normal() {
        assert_eq!(format!("{:?}", EditorMode::Normal), "Normal");
    }

    #[test]
    fn debug_formatting_insert() {
        assert_eq!(format!("{:?}", EditorMode::Insert), "Insert");
    }

    #[test]
    fn debug_formatting_visual() {
        assert_eq!(format!("{:?}", EditorMode::Visual), "Visual");
    }

    #[test]
    fn debug_formatting_command() {
        assert_eq!(format!("{:?}", EditorMode::Command), "Command");
    }

    #[test]
    fn clone_produces_equal_value() {
        let mode = EditorMode::Insert;
        let cloned = mode.clone();
        assert_eq!(mode, cloned);
    }

    #[test]
    fn copy_produces_equal_value() {
        let mode = EditorMode::Visual;
        let copied = mode; // Copy is implicit
        assert_eq!(mode, copied);
    }

    #[test]
    fn match_exhaustiveness() {
        // Ensure all variants are covered in a match
        let modes = [
            EditorMode::Normal,
            EditorMode::Insert,
            EditorMode::Visual,
            EditorMode::Command,
        ];
        for mode in modes {
            let label = match mode {
                EditorMode::Normal => "normal",
                EditorMode::Insert => "insert",
                EditorMode::Visual => "visual",
                EditorMode::Command => "command",
            };
            assert!(!label.is_empty());
        }
    }
}
