use crate::{
    api::nodes::models::NodeList,
    cmd::models::{DcCmdError, PrintFormat},
};
use colored::Colorize;

const ERROR_PREFIX: &str = "Error: ";
const SUCCESS_PREFIX: &str = "Success: ";

pub fn format_error_message(message: &str) -> String {
    let err_prefix_red = format!("{}", ERROR_PREFIX.red());

    format!("{} {}", err_prefix_red, message)
}

pub fn format_success_message(message: &str) -> String {
    let succ_prefix_green = format!("{}", SUCCESS_PREFIX.green());

    format!("{} {}", succ_prefix_green, message)
}

pub fn format_node_list(node_list: NodeList, format: PrintFormat) -> String {
    todo!()
}

fn to_readable_size(size: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB", "PB"];

    if size == 0 {
        return format!("{} {}", size, units[size as usize]);
    }

    let exp = (size as f64).log(1024.0).floor() as u64;
    let pot = 1024f64.powf(exp as f64);
    let res = size as f64 / pot as f64;

    format!("{:.0} {}", res, units[exp as usize])
}

type ParsedPath = (String, String, u64);
pub fn parse_node_path(path: &str, base_url: &str) -> Result<ParsedPath, DcCmdError> {
    let path = path.trim_start_matches(base_url);

    if path == "/" {
        return Ok((String::from("/"), String::from(""), 0));
    }

    let (parent_path, name, depth) = match path.ends_with('/') {
        // this is a container (folder or room)
        true => {
            let path = path.trim_end_matches('/');
            let path = path.split('/').collect::<Vec<&str>>();
            let name = path.last().ok_or(DcCmdError::InvalidUrl)?.to_string();
            let parent_path = path[..path.len() - 1].join("/");
            let parent_path = format!("{}/", parent_path);
            let depth = path.len() as u64 - 1;

            (parent_path, name, depth)
        }
        // this is a file
        false => {
            let path = path.split('/').collect::<Vec<&str>>();
            let name = path.last().ok_or(DcCmdError::InvalidUrl)?.to_string();
            let parent_path = path[..path.len() - 1].join("/");
            let parent_path = format!("{}/", parent_path);
            let depth = path.len() as u64 - 2;

            (parent_path, name, depth)
        }
    };

    Ok((parent_path, name, depth))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_success_message() {
        let message = "All good here.";
        assert_eq!(
            "\u{1b}[32mSuccess: \u{1b}[0m All good here.",
            format_success_message(message)
        );
    }

    #[test]
    fn test_format_error_message() {
        let message = "We have a problem.";
        assert_eq!(
            "\u{1b}[31mError: \u{1b}[0m We have a problem.",
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
        let size = 12500u64;
        assert_eq!("12 KB", to_readable_size(size));
    }

    #[test]
    fn test_to_readable_mb() {
        let size = 12500000u64;
        assert_eq!("12 MB", to_readable_size(size));
    }

    #[test]
    fn test_to_readable_gb() {
        let size = 12500000000u64;
        assert_eq!("12 GB", to_readable_size(size));
    }

    #[test]
    fn test_to_readable_tb() {
        let size = 12500000000000u64;
        assert_eq!("11 TB", to_readable_size(size));
    }

    #[test]
    fn test_to_readable_pb() {
        let size = 12500000000000000u64;
        assert_eq!("11 PB", to_readable_size(size));
    }

    #[test]
    fn test_parse_folder_path() {
        let path = "https://some.domain.com/test/folder/";
        let (parent_path, name, depth) = parse_node_path(path, "https://some.domain.com").unwrap();
        assert_eq!("/test/", parent_path);
        assert_eq!("folder", name);
        assert_eq!(2, depth);
    }

    #[test]
    fn test_file_path() {
        let path = "https://some.domain.com/test/folder/file.txt";
        let (parent_path, name, depth) = parse_node_path(path, "https://some.domain.com").unwrap();
        assert_eq!("/test/folder/", parent_path);
        assert_eq!("file.txt", name);
        assert_eq!(2, depth);
    }

    #[test]
    fn test_root_path() {
        let path = "https://some.domain.com/";
        let (parent_path, name, depth) = parse_node_path(path, "https://some.domain.com").unwrap();
        assert_eq!("/", parent_path);
        assert_eq!("", name);
        assert_eq!(0, depth);
    }
}
