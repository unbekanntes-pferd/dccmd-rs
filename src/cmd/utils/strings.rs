use std::cmp::min;

use crate::cmd::models::DcCmdError;

use dco3::nodes::models::{Node, NodeType};

use console::{style, Term};
use tracing::debug;

const ERROR_PREFIX: &str = "Error: ";
const SUCCESS_PREFIX: &str = "Success: ";

pub fn format_error_message(message: &str) -> String {
    let err_prefix_red = format!("{}", style(ERROR_PREFIX).red().bold());

    format!("{err_prefix_red} {message}")
}

pub fn format_success_message(message: &str) -> String {
    let succ_prefix_green = format!("{}", style(SUCCESS_PREFIX).green().bold());

    format!("{succ_prefix_green} {message}")
}

pub fn print_node(term: &Term, node: &Node, long: Option<bool>, human_readable: Option<bool>) {
    let mut node_str = String::new();

    let long = long.unwrap_or(false);
    let human_readable = human_readable.unwrap_or(false);

    // add metadata if long
    if long {
        // add node id
        node_str.push_str(&format!("{:<12} ", node.id));

        // add node permissions

        node_str.push_str(&format!("{} ", to_printable_permissions(node)));

        // add node updated by
        match &node.updated_by {
            Some(user_info) => node_str.push_str(&format!(
                "{:<15} {:<15} ",
                user_info
                    .first_name
                    .clone()
                    .unwrap_or_else(|| "n/a".to_string()),
                user_info
                    .last_name
                    .clone()
                    .unwrap_or_else(|| "n/a".to_string())
            )),
            None => node_str.push_str("n/a n/a"),
        }

        // add node size
        if human_readable {
            node_str.push_str(&format!("{:<8} ", to_readable_size(node.size.unwrap_or(0))));
        } else {
            node_str.push_str(&format!("{:<16} ", node.size.unwrap_or(0)));
        }

        match &node.timestamp_modification {
            Some(timestamp) => {
                node_str.push_str(&format!("{:<16} ", timestamp.format("%Y %b %e %H:%M")));
            }
            None => node_str.push_str("n/a"),
        }
    }

    // add node name
    match node.node_type {
        NodeType::File => node_str.push_str(&format!("{} ", node.name)),
        _ => node_str.push_str(&format!(
            "{:<45} ",
            style(node.name.clone()).bold().yellow()
        )),
    }

    term.write_line(&node_str)
        .expect("Could not write to terminal");
}

fn to_printable_permissions(node: &Node) -> String {
    let mut out_str = String::new();

    // add node type
    match node.node_type {
        NodeType::File => {
            out_str.push_str(&format!("{:<2}", style("--").bold()));
        }
        NodeType::Folder => {
            out_str.push_str(&format!("{:<2}", style("d-").bold()));
        }
        NodeType::Room => {
            out_str.push_str(&format!("{:<2}", style("R-").bold()));
        }
    };

    // add node permissions
    match &node.permissions {
        Some(permissions) => {
            out_str.push_str(&format!("{:<13} ", style(permissions.to_string()).bold()));
        }
        None => {
            out_str.push_str(&format!("{:<13} ", style("-------------").bold()));
        }
    };

    out_str
}

fn to_readable_size(size: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB", "PB"];

    if size == 0 {
        return "0 B".to_string();
    }

    let exp = min(
        (size.saturating_sub(1)).ilog(1024) as usize,
        units.len() - 1,
    );

    let divisor = 1u64 << (exp * 10);
    let size_whole = size / divisor;
    let size_frac = ((size % divisor) * 10 + divisor / 2) / divisor;

    if exp == 0 {
        format!("{:.0} {}", size, units[exp as usize])
    } else if size_frac == 0 {
        format!("{:.0} {}", size_whole, units[exp as usize])
    } else {
        format!("{:.0}.{} {}", size_whole, size_frac, units[exp as usize])
    }
}

type ParsedPath = (String, String, u64);
pub fn parse_path(path: &str, base_url: &str) -> Result<ParsedPath, DcCmdError> {
    let base_url = base_url.trim_start_matches("https://");
    let path = path.trim_start_matches("https://");
    let path = path.trim_start_matches(base_url).trim_start_matches('/');

    debug!("path: {}", path);

    let path_parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    debug!("path parts: {:?}", path_parts);

    let name = (*path_parts
        .last()
        .ok_or(DcCmdError::InvalidPath(path.to_string()))?)
    .to_string();
    let depth = path_parts.len().saturating_sub(1) as u64;

    let parent_path = if depth == 0 {
        String::from("/")
    } else {
        format!("/{}/", path_parts[..path_parts.len() - 1].join("/"))
    };

    debug!("parent path: {}", parent_path);
    debug!("name: {}", name);
    debug!("depth: {}", depth);

    Ok((parent_path, name, depth))
}

pub fn build_node_path(path: ParsedPath) -> String {
    let (parent_path, name, depth) = path;

    if depth == 0 {
        return format!("/{name}/");
    }

    format!("{parent_path}{name}/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_path_no_https() {
        let path = "some.domain.com/test/folder/";
        let base_url = "some.domain.com";
        let (parent_path, name, depth) = parse_path(path, base_url).unwrap();
        assert_eq!("/test/", parent_path);
        assert_eq!("folder", name);
        assert_eq!(1, depth);
    }

    #[test]
    fn test_parse_path_https() {
        let path = "https://some.domain.com/test/folder/";
        let base_url = "some.domain.com";
        let (parent_path, name, depth) = parse_path(path, base_url).unwrap();
        assert_eq!("/test/", parent_path);
        assert_eq!("folder", name);
        assert_eq!(1, depth);
    }

    #[test]
    #[ignore = "does not work without a terminal"]
    fn test_format_success_message() {
        let message = "All good here.";
        assert_eq!(
            "\u{1b}[32m\u{1b}[1mSuccess: \u{1b}[0m All good here.",
            format_success_message(message)
        );
    }

    #[test]
    #[ignore = "does not work without a terminal"]
    fn test_format_error_message() {
        let message = "We have a problem.";
        assert_eq!(
            "\u{1b}[31m\u{1b}[1mError: \u{1b}[0m We have a problem.",
            format_error_message(message)
        );
    }

    #[test]
    fn test_to_readable_zero() {
        let size = 0u64;
        assert_eq!("0 B", to_readable_size(size));
    }

    #[test]
    fn test_to_readable_b() {
        let size = 12u64;
        assert_eq!("12 B", to_readable_size(size));
    }

    #[test]
    fn test_to_readable_kb() {
        let size = 1024 * 12;
        assert_eq!("12 KB", to_readable_size(size));
    }

    #[test]
    fn test_to_readable_mb() {
        let size = 12 * 1024 * 1024;
        assert_eq!("12 MB", to_readable_size(size));
    }

    #[test]
    fn test_to_readable_gb() {
        let size = 12 * 1024 * 1024 * 1024;
        assert_eq!("12 GB", to_readable_size(size));
    }

    #[test]
    fn test_to_readable_tb() {
        let size = 12 * 1024 * 1024 * 1024 * 1024;
        assert_eq!("12 TB", to_readable_size(size));
    }

    #[test]
    fn test_to_readable_pb() {
        let size = 12 * 1024 * 1024 * 1024 * 1024 * 1024;
        assert_eq!("12 PB", to_readable_size(size));
    }

    #[test]
    fn test_parse_folder_path() {
        let path = "some.domain.com/test/folder/";
        let (parent_path, name, depth) = parse_path(path, "https://some.domain.com").unwrap();
        assert_eq!("/test/", parent_path);
        assert_eq!("folder", name);
        assert_eq!(1, depth);
    }

    #[test]
    fn test_parse_folder_path_no_trail_slash() {
        let path = "some.domain.com/test/folder";
        let (parent_path, name, depth) = parse_path(path, "https://some.domain.com").unwrap();
        assert_eq!("/test/", parent_path);
        assert_eq!("folder", name);
        assert_eq!(1, depth);
    }

    #[test]
    fn test_file_path() {
        let path = "some.domain.com/test/folder/file.txt";
        let (parent_path, name, depth) = parse_path(path, "https://some.domain.com").unwrap();
        assert_eq!("/test/folder/", parent_path);
        assert_eq!("file.txt", name);
        assert_eq!(2, depth);
    }

    #[test]
    fn test_root_path() {
        let path = "some.domain.com/";
        let (parent_path, name, depth) = parse_path(path, "https://some.domain.com").unwrap();
        assert_eq!("/", parent_path);
        assert_eq!("", name);
        assert_eq!(0, depth);
    }
}
