# node-config

This crate contains the types that represent the node's configuration. This is useful when you need to understand the 
node's config but you don't want to pull the entire node crate in.

# Adding dependencies to this crate

When adding dependencies to this crate make sure that you're not adding anything large and unnecessary as we want to 
keep it lean. Ideally, this would not pull in any dependencies that the `nillion-client` crate pulls in as probably
most crates that use this crate end up spinning a client up.
