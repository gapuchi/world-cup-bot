use crate::types::{Context, Error};

pub(crate) fn guild_id(ctx: &Context<'_>) -> Result<u64, Error> {
    Ok(ctx
        .guild_id()
        .ok_or("This command must be used in a server.")?
        .get())
}
