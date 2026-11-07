mod init;
mod kind;
mod mutation;
mod quote;

pub use init::init_script;
pub use kind::ShellKind;
pub use mutation::{ShellMutation, render_mutation};

#[cfg(test)]
mod tests;
