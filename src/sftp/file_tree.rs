use crate::models::FileKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectoryHint {
    ResolveHome,
    HomeRelative(String),
    Absolute(String),
}

pub fn parse_cd_command(current_directory: &str, command: &str) -> Option<DirectoryHint> {
    let trimmed = command.trim();

    if trimmed == "cd" || trimmed == "cd ~" {
        return Some(DirectoryHint::ResolveHome);
    }

    let target = trimmed.strip_prefix("cd ")?.trim();
    if target.is_empty() {
        return Some(DirectoryHint::ResolveHome);
    }

    let target = strip_shell_quotes(target);

    // Strip trailing shell operators so that compound commands like
    // `cd /tmp && ls` don't pollute the target path.
    let target = strip_shell_suffix(target);
    if target.is_empty() {
        return Some(DirectoryHint::ResolveHome);
    }

    // Tilde paths require home-directory resolution on the session side.
    if target == "~" {
        return Some(DirectoryHint::ResolveHome);
    }
    if let Some(sub) = target.strip_prefix("~/") {
        let sub = sub.trim_start_matches('/');
        if sub.is_empty() {
            return Some(DirectoryHint::ResolveHome);
        }
        return Some(DirectoryHint::HomeRelative(sub.to_string()));
    }

    if target == "$HOME" || target == "${HOME}" {
        return Some(DirectoryHint::ResolveHome);
    }
    if let Some(sub) = target.strip_prefix("$HOME/") {
        let sub = sub.trim_start_matches('/');
        if sub.is_empty() {
            return Some(DirectoryHint::ResolveHome);
        }
        return Some(DirectoryHint::HomeRelative(sub.to_string()));
    }
    if let Some(sub) = target.strip_prefix("${HOME}/") {
        let sub = sub.trim_start_matches('/');
        if sub.is_empty() {
            return Some(DirectoryHint::ResolveHome);
        }
        return Some(DirectoryHint::HomeRelative(sub.to_string()));
    }

    Some(DirectoryHint::Absolute(normalize_remote_path(
        current_directory,
        target,
    )))
}

fn strip_shell_quotes(target: &str) -> &str {
    let bytes = target.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &target[1..target.len() - 1];
        }
    }

    target
}

fn strip_shell_suffix(target: &str) -> &str {
    let target = target.split(';').next().unwrap_or("").trim();
    let target = target.split("&&").next().unwrap_or("").trim();
    target.split("||").next().unwrap_or("").trim()
}

pub fn normalize_remote_path(current_directory: &str, target: &str) -> String {
    if target.starts_with('/') {
        return collapse_segments(target);
    }

    if target == "." {
        return collapse_segments(current_directory);
    }

    if target == ".." {
        return collapse_segments(&format!("{current_directory}/.."));
    }

    collapse_segments(&format!("{current_directory}/{target}"))
}

pub fn collapse_segments(path: &str) -> String {
    let mut stack = Vec::new();

    for segment in path.split('/') {
        match segment {
            "" | "." => continue,
            ".." => {
                stack.pop();
            }
            other => stack.push(other),
        }
    }

    if stack.is_empty() {
        "/".into()
    } else {
        format!("/{}", stack.join("/"))
    }
}

pub fn format_permissions(mode: Option<u32>) -> String {
    let Some(mode) = mode else {
        return "---------".into();
    };

    let file_kind = match mode & 0o170000 {
        0o040000 => 'd',
        0o120000 => 'l',
        _ => '-',
    };

    let mut rendered = String::with_capacity(10);
    rendered.push(file_kind);

    for shift in [6_u32, 3, 0] {
        rendered.push(if mode & (0o4 << shift) != 0 { 'r' } else { '-' });
        rendered.push(if mode & (0o2 << shift) != 0 { 'w' } else { '-' });
        rendered.push(if mode & (0o1 << shift) != 0 { 'x' } else { '-' });
    }

    rendered
}

pub fn infer_kind(mode: Option<u32>) -> FileKind {
    match mode.unwrap_or_default() & 0o170000 {
        0o040000 => FileKind::Directory,
        0o120000 => FileKind::Symlink,
        0o100000 => FileKind::File,
        _ => FileKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::{DirectoryHint, collapse_segments, format_permissions, parse_cd_command};

    #[test]
    fn collapses_relative_segments() {
        assert_eq!(
            collapse_segments("/var/www/../log/./nginx"),
            "/var/log/nginx"
        );
    }

    #[test]
    fn parses_cd_into_absolute_path() {
        assert_eq!(
            parse_cd_command("/srv/apps", "cd ../logs"),
            Some(DirectoryHint::Absolute("/srv/logs".into()))
        );
    }

    #[test]
    fn parses_cd_tilde_relative() {
        assert_eq!(
            parse_cd_command("/srv/apps", "cd ~/Documents"),
            Some(DirectoryHint::HomeRelative("Documents".into()))
        );
    }

    #[test]
    fn parses_cd_with_compound_command() {
        assert_eq!(
            parse_cd_command("/srv/apps", "cd /tmp && ls"),
            Some(DirectoryHint::Absolute("/tmp".into()))
        );
        assert_eq!(
            parse_cd_command("/srv/apps", "cd /var; echo done"),
            Some(DirectoryHint::Absolute("/var".into()))
        );
        assert_eq!(
            parse_cd_command("/srv/apps", "cd /opt || cd /tmp"),
            Some(DirectoryHint::Absolute("/opt".into()))
        );
    }

    #[test]
    fn formats_permissions() {
        assert_eq!(format_permissions(Some(0o100644)), "-rw-r--r--");
    }
}
