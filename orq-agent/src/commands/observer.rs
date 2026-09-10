use color_eyre::eyre::Result;

pub(crate) use crate::observer::{
    ObserverEmitArgs, ObserverEmitReport, ObserverSnapshotArgs, ObserverSnapshotReport,
};

pub(crate) async fn run_emit(args: ObserverEmitArgs) -> Result<ObserverEmitReport> {
    crate::observer::emit(args).await
}

pub(crate) async fn run_snapshot(args: ObserverSnapshotArgs) -> Result<ObserverSnapshotReport> {
    crate::observer::snapshot(args).await
}
