use crate::{
    api::nodes::models::{Node, NodeType},
    cmd::models::DcCmdError,
};

use chrono::{DateTime, Utc};
use console::{style, Term};
use tracing::{debug, trace};

const ERROR_PREFIX: &str = "Error: ";
const SUCCESS_PREFIX: &str = "Success: ";

const NODE_LIST_HEADER: &str =
    "id\tname\ttype\tsize\tmodified\tcreated\tparent id\tparent path\tDe";

pub fn format_error_message(message: &str) -> String {
    let err_prefix_red = format!("{}", style(ERROR_PREFIX).red().bold());

    format!("{err_prefix_red} {message}")
}

pub fn format_success_message(message: &str) -> String {
    let succ_prefix_green = format!("{}", style(SUCCESS_PREFIX).green().bold());

    format!("{succ_prefix_green} {message}")
}

pub fn print_node(
    term: &Term,
    node: &Node,
    long: Option<bool>,
    human_readable: Option<bool>,
) {
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
                user_info.first_name.clone().unwrap_or("n/a".to_string()),
                user_info.last_name.clone().unwrap_or("n/a".to_string())
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
                let dt: DateTime<Utc> = DateTime::parse_from_rfc3339(timestamp)
                    .expect("Malformed date")
                    .into();
                node_str.push_str(&format!("{:<16} ", dt.format("%Y %b %e %H:%M")));
            }
            None => node_str.push_str("n/a"),
        }
    }

    // add node name
    match node.node_type {
        NodeType::File => node_str.push_str(&format!("{} ", node.name)),
        _ => node_str.push_str(&format!("{:<45} ", style(node.name.clone()).bold().yellow())),
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

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn to_readable_size(size: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB", "PB"];

    if size == 0 {
        // size is 0, so this is safe
        return format!("{} {}", size, units[size as usize]);
    }

    // size is always positive, so this is safe
    let exp = (size as f64).log(1024.0).floor() as u64;

    // precision loss is ok here because we are only interested in the integer part
    let pot = 1024f64.powf(exp as f64);

    // precision loss is ok here because we are only interested in the integer part
    let res = size as f64 / pot;
    
    // exp is always positive, so this is safe 
    format!("{:.0} {}", res, units[exp as usize])
}

type ParsedPath = (String, String, u64);
pub fn parse_path(path: &str, base_url: &str) -> Result<ParsedPath, DcCmdError> {
    let base_url = base_url.trim_start_matches("https://");
    let path = path.trim_start_matches(&base_url);
    let path = path.trim_start_matches("/");

    debug!("path: {}", path);

    if path == "/" {
        return Ok((String::from("/"), String::new(), 0));
    }
    
    let path_parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    debug!("path_parts: {:?}", path_parts);
    let name = String::from(*path_parts.last().ok_or(DcCmdError::InvalidPath(path.to_string()))?);
    let mut parent_path = format!("/{}/", path_parts[..path_parts.len() - 1].join("/"));
    let depth = path_parts.iter().count().checked_sub(1).unwrap_or(0) as u64; 

    debug!("parent_path: {}", parent_path);
    debug!("name: {}", name);
    debug!("depth: {}", depth);

    if parent_path == "//".to_owned() {
        if let Some((prefix, _)) = parent_path.rsplit_once('/') {
        parent_path = prefix.to_owned();
    }
}

    Ok((parent_path, name, depth))
}


pub fn parse_niode_path(path: &str, base_url: &str) -> Result<ParsedPath, DcCmdError> {
    
    let base_url = base_url.trim_start_matches("https://");
    let path = path.trim_start_matches(&base_url);

    debug!("path: {}", path);

    if path == "/" {
        return Ok((String::from("/"), String::new(), 0));
    }

    let (parent_path, name, depth) = if path.ends_with('/') {
        // this is a container (folder or room)
         
     
            debug!("path: {}", path);
            let path = path.split('/').collect::<Vec<&str>>();
            debug!("path: {:?}", path);
            let name = (*path.last().ok_or(DcCmdError::InvalidPath(path.clone().join("/")))?).to_string();
            debug!("name: {}", name);
            let parent_path = path[..path.len() - 1].join("/");
            debug!("parent_path: {}", parent_path);
            let parent_path = format!("{parent_path}/");
            debug!("parent_path: {}", parent_path);
            let parent_path = parent_path.trim_start_matches(&base_url).to_string();
            debug!("parent_path: {}", parent_path);
            let depth = path.len() as u64 - 2;

            (parent_path, name, depth)
        }
        // this is a file
        else {
            let path = path.split('/').collect::<Vec<&str>>();
            let name = (*path.last().ok_or(DcCmdError::InvalidPath(path.clone().join("/")))?).to_string();
            let parent_path = path[..path.len() - 1].join("/");
            let parent_path = format!("{parent_path}/");
            let parent_path = parent_path.trim_start_matches(&base_url).to_string();
            let depth = path.len() as u64 - 2;

            (parent_path, name, depth)
        }
    ;

    debug!("parent_path: {}", parent_path);
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
        let size = 12500_u64;
        assert_eq!("12 KB", to_readable_size(size));
    }

    #[test]
    fn test_to_readable_mb() {
        let size = 12_500_000_u64;
        assert_eq!("12 MB", to_readable_size(size));
    }

    #[test]
    fn test_to_readable_gb() {
        let size = 12_500_000_000_u64;
        assert_eq!("12 GB", to_readable_size(size));
    }

    #[test]
    fn test_to_readable_tb() {
        let size = 12_500_000_000_000_u64;
        assert_eq!("11 TB", to_readable_size(size));
    }

    #[test]
    fn test_to_readable_pb() {
        let size = 12_500_000_000_000_000_u64;
        assert_eq!("11 PB", to_readable_size(size));
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
