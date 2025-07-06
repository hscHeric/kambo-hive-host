use kambo_hive::host::{
    periodic_saver, result_aggregator::ResultAggregator, task_manager::TaskManager,
};
use kambo_hive::utils::{init_logger, listen_for_workers};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::{env, fs, process, sync::Arc};
use tokio::sync::Mutex;

#[derive(Serialize, Deserialize)]
pub struct GAConfig {
    pub trials: u32,
    pub max_stagnant: usize,
    pub generations: usize,
    pub tournament_size: usize,
    pub crossover_probability: f32,
    pub pop_size: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logger();
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!(
            "Uso: {} <bind_addr:port> <graphs_path> [save_path] [save_interval_secs]",
            args[0]
        );
        eprintln!(
            "Exemplo: {} 0.0.0.0:12345 ./graphs results.json 60",
            args[0]
        );
        process::exit(1);
    }

    let bind_addr = &args[1];
    let graphs_path = &args[2];
    let save_path = args.get(3);
    let save_interval = args.get(4).and_then(|s| s.parse().ok());

    let task_manager = Arc::new(Mutex::new(TaskManager::new()));
    let result_aggregator = Arc::new(Mutex::new(ResultAggregator::new()));

    let ga_config = GAConfig {
        trials: 10,
        max_stagnant: 100,
        generations: 1000,
        tournament_size: 2,
        crossover_probability: 0.9,
        pop_size: None,
    };
    let ag_config_str = serde_json::to_string(&ga_config)?;
    info!("Lendo grafos de: {}", graphs_path);
    let paths = fs::read_dir(graphs_path)?;
    let mut tm = task_manager.lock().await;
    for path in paths {
        let path = path?.path();
        if path.is_file() {
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                info!("Adicionando tasks para o grafo: {}", file_name);
                tm.add_new_graph_tasks(file_name, ga_config.trials, &ag_config_str);
            }
        }
    }
    info!("Total de {} tarefas adicionadas.", tm.get_total_tasks());
    drop(tm);

    let addr_clone = bind_addr.clone();
    tokio::spawn(async move {
        listen_for_workers(addr_clone).await;
    });

    if let Some(path) = save_path {
        let interval = save_interval.unwrap_or(300);
        periodic_saver::start(Arc::clone(&result_aggregator), path.clone(), interval);
    } else {
        warn!("Salvamento periódico desativado. Forneça um caminho e intervalo para ativar.");
    }

    info!("Host TCP escutando em {}", bind_addr);
    if let Err(e) =
        kambo_hive::host::server::start_server(bind_addr, task_manager, result_aggregator).await
    {
        error!("Erro no servidor: {}", e);
    }

    Ok(())
}
