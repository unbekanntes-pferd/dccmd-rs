#![allow(clippy::module_name_repetitions)]
#![allow(clippy::struct_excessive_bools)]

use clap::Parser;
use cmd::{
    config::{handle_config_cmd, logs::init_logging},
    groups::handle_groups_cmd,
    handle_error,
    models::{DcCmd, DcCmdCommand, ListOptions, PasswordAuth},
    nodes::{
        copy_nodes, create_folder, create_room, delete_node,
        download::download,
        list_nodes,
        models::{
            CmdCopyOptions, CmdDownloadOptions, CmdListNodesOptions, CmdMkRoomOptions,
            CmdTransferOptions, CmdUploadOptions,
        },
        transfer::transfer_node,
        upload::upload,
    },
    print_version,
    reports::handle_reports_cmd,
    users::handle_users_cmd,
};
use console::Term;

mod cmd;

#[allow(clippy::too_many_lines)]
#[tokio::main]
async fn main() {
    let opt = DcCmd::parse();

    let term = Term::stdout();
    let err_term = Term::stderr();

    init_logging(&err_term, opt.debug, opt.log_file_path);

    let password_auth = match (opt.username, opt.password) {
        (Some(username), Some(password)) => Some(PasswordAuth(username, password)),
        _ => None,
    };

    let res = match opt.cmd {
        DcCmdCommand::Download {
            source,
            target,
            velocity,
            recursive,
            share_password,
            include_rooms,
        } => {
            download(
                source,
                target,
                CmdDownloadOptions::new(
                    recursive,
                    velocity,
                    password_auth,
                    opt.encryption_password,
                    share_password,
                    include_rooms,
                ),
            )
            .await
        }
        DcCmdCommand::Upload {
            source,
            target,
            overwrite,
            keep_share_links,
            classification,
            velocity,
            recursive,
            skip_root,
            share,
            share_password,
        } => {
            upload(
                term,
                source.into(),
                target,
                CmdUploadOptions::new(
                    overwrite,
                    keep_share_links,
                    recursive,
                    skip_root,
                    share,
                    classification,
                    velocity,
                    password_auth,
                    opt.encryption_password,
                    share_password,
                ),
            )
            .await
        }
        DcCmdCommand::Transfer {
            source,
            target,
            overwrite,
            keep_share_links,
            classification,
            share,
            share_password,
        } => {
            transfer_node(
                term,
                source,
                target,
                CmdTransferOptions::new(
                    overwrite,
                    keep_share_links,
                    share,
                    classification,
                    share_password,
                ),
            )
            .await
        }
        DcCmdCommand::Ls {
            source,
            filter,
            long,
            human_readable,
            managed,
            all,
            offset,
            limit,
        } => {
            let list_opts = ListOptions::new(filter, offset, limit, all, false);
            let opts =
                CmdListNodesOptions::new(list_opts, human_readable, long, managed, password_auth);

            list_nodes(term, source, opts).await
        }
        DcCmdCommand::Cp { source, target } => {
            let opts = CmdCopyOptions::new(password_auth);
            copy_nodes(term, source, target, opts).await
        }
        DcCmdCommand::Mkdir {
            source,
            classification,
            notes,
        } => create_folder(term, source, classification, notes, password_auth).await,
        DcCmdCommand::Mkroom {
            inherit_permissions,
            source,
            classification,
            admin_users,
        } => {
            create_room(
                term,
                source,
                CmdMkRoomOptions::new(
                    inherit_permissions,
                    classification,
                    password_auth,
                    admin_users,
                ),
            )
            .await
        }
        DcCmdCommand::Rm { source, recursive } => {
            delete_node(term, source, Some(recursive), password_auth).await
        }
        DcCmdCommand::Users { cmd } => handle_users_cmd(cmd, term).await,
        DcCmdCommand::Groups { cmd } => handle_groups_cmd(cmd, term).await,
        DcCmdCommand::Version => print_version(&term),
        DcCmdCommand::Config { cmd } => handle_config_cmd(cmd, term).await,
        DcCmdCommand::Reports { cmd } => handle_reports_cmd(cmd, term).await,
    };

    if let Err(e) = res {
        handle_error(&err_term, &e);
    }
}
