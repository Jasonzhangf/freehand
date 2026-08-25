use std::collections::BTreeSet;

use freehand_contracts::SessionId;
use freehand_reason::{
    MAX_SESSION_LIST_PAGE_LIMIT, PersistedSessionSummary, ReasonPersistence,
    ReasonPersistenceError, ReasonSessionLatestStatus, ReasonSessionListCursor,
    ReasonSessionListPageRequest,
};
use freehand_ui_protocol::{
    UiCommandDispatchPortError, UiSessionListPageDirection, UiSessionListPageRequest,
    UiSessionSummary,
};
use serde_json::from_str;

pub(crate) fn visible_session_list_page(
    persistence: &ReasonPersistence,
    mut request: ReasonSessionListPageRequest,
) -> Result<freehand_reason::ReasonSessionListPage, ReasonPersistenceError> {
    let wanted = request.limit;
    request.limit = MAX_SESSION_LIST_PAGE_LIMIT;
    let mut visible = Vec::new();
    let mut unavailable = BTreeSet::new();
    loop {
        let page = persistence.list_persisted_sessions_page(&request)?;
        for summary in page.sessions {
            if !internal_runtime_session_id(&summary.session_id) {
                visible.push(summary);
            }
        }
        unavailable.extend(
            page.unavailable_sessions
                .into_iter()
                .filter(|session_id| !internal_runtime_session_id(session_id)),
        );
        if visible.len() > wanted || !page.has_older {
            let has_older = visible.len() > wanted;
            let next_cursor = has_older.then(|| {
                let last = &visible[wanted - 1];
                ReasonSessionListCursor {
                    last_activity_unix_seconds: last.activity_unix_seconds,
                    last_session_id: last.session_id.clone(),
                }
            });
            return Ok(freehand_reason::ReasonSessionListPage {
                has_older,
                next_cursor,
                sessions: visible.into_iter().take(wanted).collect(),
                unavailable_sessions: unavailable.into_iter().collect(),
            });
        }
        let Some(cursor) = page.next_cursor else {
            return Ok(freehand_reason::ReasonSessionListPage {
                has_older: false,
                next_cursor: None,
                sessions: visible.into_iter().take(wanted).collect(),
                unavailable_sessions: unavailable.into_iter().collect(),
            });
        };
        request.cursor = Some(cursor);
    }
}

pub(crate) fn session_list_page_request_from_ui(
    archived: bool,
    page: &UiSessionListPageRequest,
) -> Result<ReasonSessionListPageRequest, UiCommandDispatchPortError> {
    if !(1..=MAX_SESSION_LIST_PAGE_LIMIT).contains(&page.limit) {
        return Err(UiCommandDispatchPortError::DispatchFailed(format!(
            "session list page limit must be between 1 and {MAX_SESSION_LIST_PAGE_LIMIT}"
        )));
    }
    let cursor = match (&page.direction, &page.cursor) {
        (UiSessionListPageDirection::Latest, None) => None,
        (UiSessionListPageDirection::Older, Some(cursor)) => Some(
            from_str::<ReasonSessionListCursor>(cursor).map_err(|error| {
                UiCommandDispatchPortError::DispatchFailed(format!(
                    "invalid session list page cursor: {error}"
                ))
            })?,
        ),
        _ => {
            return Err(UiCommandDispatchPortError::DispatchFailed(
                "session list page direction and cursor do not match".to_owned(),
            ));
        }
    };
    Ok(ReasonSessionListPageRequest {
        archived,
        cursor,
        limit: page.limit,
    })
}

pub(crate) fn session_summary_to_ui(summary: PersistedSessionSummary) -> UiSessionSummary {
    UiSessionSummary {
        session_id: summary.session_id,
        activity_unix_seconds: summary.activity_unix_seconds,
        title: summary.title,
        archived: summary.archived,
        cwd: summary.cwd,
        latest_turn_id: summary.latest_turn_id,
        active_turn_id: summary.active_turn_id,
        turn_count: summary.turn_count,
        latest_status: reason_session_latest_status_string(summary.latest_status),
        latest_summary: summary.latest_summary,
    }
}

pub(crate) fn internal_runtime_session_id(session_id: &SessionId) -> bool {
    let id = session_id.as_str();
    id.starts_with("worker-task-")
        || id.starts_with("master-lifecycle-")
        || id.starts_with("master-timer-")
}

fn reason_session_latest_status_string(status: ReasonSessionLatestStatus) -> String {
    match status {
        ReasonSessionLatestStatus::Empty => "empty".to_owned(),
        ReasonSessionLatestStatus::WaitingModel => "waiting_model".to_owned(),
        ReasonSessionLatestStatus::ToolRunning => "tool_running".to_owned(),
        ReasonSessionLatestStatus::Terminal(status) => format!("{status:?}").to_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_page_request_rejects_zero_limit_before_paging_arithmetic() {
        let error = session_list_page_request_from_ui(
            false,
            &UiSessionListPageRequest {
                direction: UiSessionListPageDirection::Latest,
                cursor: None,
                limit: 0,
            },
        )
        .expect_err("zero session list limit must fail");
        let UiCommandDispatchPortError::DispatchFailed(message) = error else {
            panic!("unexpected session list limit error: {error:?}");
        };
        assert!(message.contains("session list page limit must be between 1 and 100"));
    }

    #[test]
    fn session_page_request_preserves_valid_latest_request() {
        let request = session_list_page_request_from_ui(
            false,
            &UiSessionListPageRequest {
                direction: UiSessionListPageDirection::Latest,
                cursor: None,
                limit: 2,
            },
        )
        .expect("valid latest session list request");
        assert!(!request.archived);
        assert_eq!(request.limit, 2);
        assert!(request.cursor.is_none());
    }
}
