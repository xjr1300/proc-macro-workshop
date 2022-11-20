use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Builder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let _input = parse_macro_input!(input as DeriveInput);

    let expanded = quote! {
        pub struct CommandBuilder {
            executable: Option<String>,
            args: Option<Vec<String>>,
            env: Option<Vec<String>>,
            current_dir: Option<String>,
        }

        impl CommandBuilder {
            fn executable(&mut self, executable: String) -> &mut Self {
                self.executable = Some(executable);
                self
            }

            fn args(&mut self, args: Vec<String>) -> &mut Self {
                self.args = Some(args);
                self
            }

            fn env(&mut self, env: Vec<String>) -> &mut Self {
                self.env = Some(env);
                self
            }

            fn current_dir(&mut self, current_dir: String) -> &mut Self {
                self.current_dir = Some(current_dir);
                self
            }

            pub fn build(&mut self) -> Result<Command, Box<dyn std::error::Error>> {
                Ok(Command {
                    executable: self.executable.take().ok_or_else(|| String::from("executable is none"))?,
                    args: self.args.take().ok_or_else(|| String::from("env args is none"))?,
                    env: self.env.take().ok_or_else(|| String::from("env is none"))?,
                    current_dir: self.current_dir.take().ok_or_else(|| String::from("current_dir is none"))?,
                })
            }
        }

        impl Command {
            fn builder() -> CommandBuilder {
                CommandBuilder {
                    executable: None,
                    args: None,
                    env: None,
                    current_dir: None,
                }
            }
        }
    };

    proc_macro::TokenStream::from(expanded)
}
