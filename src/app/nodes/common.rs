use dco3::nodes::NodeList;

use crate::{
    app::nodes::api::NodesApi,
    core::models::{build_params, DcCmdError, ListOptions},
};

const PAGE_SIZE: u64 = 500;

#[derive(Debug, Clone, Copy, Default)]
pub struct NodesPathPaginationHelper;

impl NodesPathPaginationHelper {
    pub fn new() -> Self {
        Self
    }

    pub async fn resolve_parent_id<A: NodesApi>(
        &self,
        api: &A,
        node_path: Option<&str>,
    ) -> Result<Option<u64>, DcCmdError> {
        let parent_id = if let Some(node_path) = node_path {
            let node = api.get_node_from_path(node_path).await?;

            let Some(node) = node else {
                return Err(DcCmdError::InvalidPath(node_path.to_string()));
            };

            Some(node.id)
        } else {
            None
        };

        Ok(parent_id)
    }

    pub async fn get_nodes_with_opts<A: NodesApi>(
        &self,
        api: &A,
        parent_id: Option<u64>,
        managed: Option<bool>,
        opts: &ListOptions,
    ) -> Result<NodeList, DcCmdError> {
        let offset = opts.offset().unwrap_or(0);
        let limit = u64::from(opts.limit().unwrap_or(PAGE_SIZE as u32))
            .try_into()
            .map_err(|_| {
                DcCmdError::InvalidArgument("Limit must be a positive integer.".to_string())
            })?;

        let params = build_params(opts.filter(), offset, Some(limit))?;

        let mut node_list = api.get_nodes(parent_id, managed, Some(params)).await?;

        if opts.all() && node_list.range.total > PAGE_SIZE {
            let mut page_offset = PAGE_SIZE;
            while page_offset <= node_list.range.total {
                let params = build_params(opts.filter(), page_offset, None)?;
                let next_page = api.get_nodes(parent_id, managed, Some(params)).await?;
                node_list.items.extend(next_page.items);
                page_offset += PAGE_SIZE;
            }
        }

        Ok(node_list)
    }

    pub async fn search_nodes_with_opts<A: NodesApi>(
        &self,
        api: &A,
        search_string: &str,
        parent_id: Option<u64>,
        depth_level: Option<i8>,
        opts: &ListOptions,
    ) -> Result<NodeList, DcCmdError> {
        let params = build_params(
            opts.filter(),
            opts.offset().unwrap_or(0),
            Some(
                u64::from(opts.limit().unwrap_or(PAGE_SIZE as u32))
                    .try_into()
                    .map_err(|_| {
                        DcCmdError::InvalidArgument("Limit must be a positive integer.".to_string())
                    })?,
            ),
        )?;

        let mut node_list = api
            .search_nodes(search_string, parent_id, depth_level, Some(params))
            .await?;

        if opts.all() && node_list.range.total > PAGE_SIZE {
            let mut page_offset = PAGE_SIZE;
            while page_offset <= node_list.range.total {
                let params = build_params(opts.filter(), page_offset, None)?;
                let next_page = api
                    .search_nodes(search_string, parent_id, depth_level, Some(params))
                    .await?;
                node_list.items.extend(next_page.items);
                page_offset += PAGE_SIZE;
            }
        }

        Ok(node_list)
    }

    pub async fn search_nodes_all_pages<A: NodesApi>(
        &self,
        api: &A,
        search_string: &str,
        parent_id: Option<u64>,
        depth_level: Option<i8>,
    ) -> Result<NodeList, DcCmdError> {
        let no_filter: Option<String> = None;

        let mut node_list = api
            .search_nodes(
                search_string,
                parent_id,
                depth_level,
                Some(build_params(&no_filter, 0, Some(PAGE_SIZE as u32))?),
            )
            .await?;

        if node_list.range.total > PAGE_SIZE {
            let mut page_offset = PAGE_SIZE;
            while page_offset <= node_list.range.total {
                let next_page = api
                    .search_nodes(
                        search_string,
                        parent_id,
                        depth_level,
                        Some(build_params(&no_filter, page_offset, None)?),
                    )
                    .await?;
                node_list.items.extend(next_page.items);
                page_offset += PAGE_SIZE;
            }
        }

        Ok(node_list)
    }
}

pub fn is_search_query(query: &str) -> bool {
    query.contains('*')
}
