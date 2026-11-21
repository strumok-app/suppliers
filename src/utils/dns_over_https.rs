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

#[derive(Debug, Default, Clone)]
pub struct DoHResolver {
    state: Arc<OnceLock<TokioResolver>>,
}

impl DoHResolver {
    fn init_resolver(&self) -> TokioResolver {
        TokioResolver::builder_tokio()
            .and_then(|builder| builder.build())
            .unwrap_or_else(|_| {
                Resolver::builder_with_config(
                    ResolverConfig::udp_and_tcp(&GOOGLE),
                    TokioRuntimeProvider::default(),
                )
                .build()
                .expect("failed to build fallback DNS resolver")
            })
    }
}

impl Resolve for DoHResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let hickory_resolver = self.clone();

        Box::pin(async move {
            let resolver = hickory_resolver
                .state
                .get_or_init(|| hickory_resolver.init_resolver());

            let lookup = resolver.lookup_ip(name.as_str()).await?;

            let addrs: Addrs = Box::new(lookup.into_iter().map(|addr| SocketAddr::new(addr, 0)));

            Ok(addrs)
        })
    }
}
