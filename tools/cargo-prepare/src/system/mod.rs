pub mod linux;
pub mod macos;
pub mod windows;

use crate::context::Context;

pub fn ensure(ctx: &mut Context) {
    match ctx.platform.os {
        crate::platform::Os::Linux => linux::ensure(ctx),
        crate::platform::Os::Macos => macos::ensure(ctx),
        crate::platform::Os::Windows => windows::ensure(ctx),
        crate::platform::Os::Other => {
            ctx.unsupported(
                crate::context::Target::Common,
                "Host operating system",
                "cargo prepare supports Linux, macOS, and Windows",
            );
        }
    }
}
