use hickory_resolver::Resolver;
use hickory_resolver::TokioResolver;
use hickory_resolver::config::{GOOGLE, ResolverConfig};
use hickory_resolver::net::runtime::TokioRuntimeProvider;
use reqwest::dns::Addrs;
use reqwest::dns::Name;
use reqwest::dns::Resolve;
use reqwest::dns::Resolving;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::OnceLock;

/// DNS resolver that always queries Google's public resolvers over
/// DNS-over-HTTPS (`https://dns.google`) on every platform.
///
/// It deliberately never reads the system DNS configuration: on Android the
/// bundle is loaded through plain FFI without initializing `ndk-context`, so
/// hickory's `system-config` path panics instead of returning an error. By
/// building the resolver from a fixed `ResolverConfig::https(&GOOGLE)` config
/// (servers given by IP with the TLS name `dns.google`, so there is no
/// bootstrap-DNS problem) we avoid any system lookup and any panic.
#[derive(Debug, Default, Clone)]
pub struct DoHResolver {
    state: Arc<OnceLock<TokioResolver>>,
}

impl DoHResolver {
    fn build_resolver() -> Result<TokioResolver, hickory_resolver::net::NetError> {
        Resolver::builder_with_config(
            ResolverConfig::https(&GOOGLE),
            TokioRuntimeProvider::default(),
        )
        .build()
    }
}

impl Resolve for DoHResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let doh_resolver = self.clone();

        Box::pin(async move {
            let resolver = match doh_resolver.state.get() {
                Some(resolver) => resolver,
                None => {
                    let built = DoHResolver::build_resolver()?;
                    // Another task may have initialized it concurrently; either
                    // way `get` will return the stored resolver afterwards.
                    let _ = doh_resolver.state.set(built);
                    doh_resolver
                        .state
                        .get()
                        .expect("resolver was just initialized")
                }
            };

            let lookup = resolver.lookup_ip(name.as_str()).await?;

            let addrs: Addrs = Box::new(lookup.into_iter().map(|addr| SocketAddr::new(addr, 0)));

            Ok(addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_resolver::config::ProtocolConfig;

    #[test_log::test(tokio::test)]
    async fn resolves_real_hostname_via_doh() {
        let resolver = DoHResolver::default();
        let name: Name = "api.themoviedb.org".parse().unwrap();

        let addrs: Vec<SocketAddr> = resolver.resolve(name).await.unwrap().collect();

        assert!(
            !addrs.is_empty(),
            "expected at least one address for api.themoviedb.org"
        );
    }

    #[test]
    fn google_config_uses_only_https_nameservers() {
        let config = ResolverConfig::https(&GOOGLE);
        let name_servers = config.name_servers();

        assert!(!name_servers.is_empty());
        assert_eq!(
            name_servers.len(),
            GOOGLE.ips.len(),
            "config must contain exactly Google's servers"
        );

        for ns in name_servers {
            assert!(
                GOOGLE.ips.contains(&ns.ip),
                "unexpected name server ip: {}",
                ns.ip
            );
            assert!(!ns.connections.is_empty());
            for conn in &ns.connections {
                match &conn.protocol {
                    ProtocolConfig::Https { server_name, .. } => {
                        assert_eq!(server_name.as_ref(), "dns.google");
                    }
                    other => panic!("expected only DNS-over-HTTPS, got {other:?}"),
                }
            }
        }
    }
}
