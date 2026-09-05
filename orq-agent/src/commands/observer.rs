use color_eyre::eyre::Result;

pub(crate) use crate::observer::{ObserverEmitArgs, ObserverEmitReport};

pub(crate) async fn run_emit(args: ObserverEmitArgs) -> Result<ObserverEmitReport> {
    crate::observer::emit(args).await
}
