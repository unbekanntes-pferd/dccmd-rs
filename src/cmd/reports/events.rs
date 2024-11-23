use crate::cmd::models::DcCmdError;
use dco3::{eventlog::LogEventList, Eventlog};

use super::{models::EventOptions, ReportsCommandHandler};

impl ReportsCommandHandler {
    pub async fn get_events(&self, opts: EventOptions) -> Result<LogEventList, DcCmdError> {
        let params = opts.clone().into();

        let mut event_list = self.client.eventlog().get_events(params).await?;

        if opts.list_options.all() {
            let reqs = (500..=event_list.range.total)
                .step_by(500)
                .map(|offset| {
                    let params = opts.new_params_with_offset(offset);
                    self.client.eventlog().get_events(params)
                })
                .collect::<Vec<_>>();

            for req in reqs {
                let next_events = req.await?;
                event_list.items.extend(next_events.items);
            }
        }

        Ok(event_list)
    }
}
