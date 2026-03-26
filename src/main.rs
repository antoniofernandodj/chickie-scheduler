mod jobs;

use anyhow::Result;
use chrono::Utc;
use cron::Schedule;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::str::FromStr; // ← Necessário para Schedule::from_str
use futures::future::join_all; // ← Para aguardar todas as tasks
use std::sync::Arc;
use tokio::signal;
use tokio::time::{sleep, Duration};
use tracing::{info, error, Level, warn};
use tracing_subscriber::FmtSubscriber;


use jobs::{CronJob, backup::BackupJob, cleanup::CleanupJob, health_check::HealthCheckJob};

#[derive(Debug, Deserialize)]
struct JobSchedule {
    name: String,
    schedule: String,
    enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct Config {
    jobs: Vec<JobSchedule>,
}

fn get_config_path() -> String {
    // 1. Prioridade máxima: variável de ambiente CONFIG_PATH
    if let Ok(path) = env::var("CONFIG_PATH") {
        return path;
    }
    
    // 2. Se estiver em Docker, usa o path padrão do container
    if std::path::Path::new("/app/config.toml").exists() {
        return "/app/config.toml".to_string();
    }
    
    // 3. Fallback para desenvolvimento local
    "config.toml".to_string()
}

fn load_config(path: &str) -> Result<Config> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

fn create_job_registry() -> HashMap<&'static str, Arc<dyn CronJob>> {
    let mut registry: HashMap<&'static str, Arc<dyn CronJob>> = HashMap::new();
    registry.insert("backup_job", Arc::new(BackupJob));
    registry.insert("cleanup_job", Arc::new(CleanupJob));
    registry.insert("health_check_job", Arc::new(HealthCheckJob));
    registry
}

async fn run_scheduled_job(
    job: Arc<dyn CronJob>,
    schedule: Schedule,
) -> Result<()> {
    let job_name = job.name();
    info!("⏰ Job '{}' registrado com schedule: {}", job_name, schedule);

    loop {
        let now = Utc::now();
        
        // ✅ CORREÇÃO: usa `after()` que é público, ou `upcoming().next()`
        let next = schedule
            .upcoming(Utc)
            .next()
            .expect("Erro ao calcular próxima execução");
        
        let duration = (next - now)
            .to_std()
            .unwrap_or(Duration::from_secs(0));

        info!("⏳ Job '{}' aguardando até {} (em {:?})", job_name, next, duration);
        sleep(duration).await;

        info!("🚀 Executando job '{}'...", job_name);
        
        let start = std::time::Instant::now();
        match job.execute().await {
            Ok(_) => {
                let elapsed = start.elapsed();
                info!("✅ Job '{}' concluído em {:?}", job_name, elapsed);
            }
            Err(e) => {
                error!("❌ Job '{}' falhou: {}", job_name, e);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_env_filter("chickie_scheduler=info") // ← Ajustado para nome do seu crate
        .init();

    info!("🔧 Rust Cron Scheduler iniciando...");

    let config_path = get_config_path();
    info!("🔧 Chickie Scheduler iniciando...");
    info!("📄 Carregando config de: {}", config_path);

    let config = match load_config(&config_path) {
        Ok(c) => c,
        Err(e) => {
            error!("Falha crítica ao carregar config '{}': {}", config_path, e);
            return Ok(());
        }
    };

    let registry = create_job_registry();
    let mut futures = vec![];

    for job_config in config.jobs {
        if job_config.enabled == Some(false) {
            info!("⏭️  Job '{}' desabilitado, ignorando...", job_config.name);
            continue;
        }

        match (Schedule::from_str(&job_config.schedule), registry.get(job_config.name.as_str())) {
            (Ok(schedule), Some(job)) => {
                let job_clone = Arc::clone(job);
                // Spawn retorna um JoinHandle que podemos await depois
                let handle = tokio::spawn(run_scheduled_job(job_clone, schedule));
                futures.push(handle);
                info!("✅ '{}' agendado", job_config.name);
            }
            (Err(e), _) => error!("❌ Schedule inválido para '{}': {}", job_config.name, e),
            (_, None) => error!("⚠️  Job '{}' não registrado no código", job_config.name),
        }
    }

    if futures.is_empty() {
        warn!("⚠️  Nenhum job ativo. Mantendo processo vivo...");
        // Mantém o processo vivo mesmo sem jobs
        signal::ctrl_c().await?;
        return Ok(());
    }

    info!("🎯 {} jobs rodando. Pressione Ctrl+C para parar.", futures.len());

    // ✅ BLOQUEIO REAL:
    // Escolhe entre: receber sinal de parada OU todas as jobs terminarem (o que não deve acontecer num loop infinito)
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("🛑 Sinal de parada recebido. Encerrando...");
        }
        _ = join_all(futures) => {
            info!("🏁 Todas as jobs finalizaram (inesperado).");
        }
    }

    Ok(())
}