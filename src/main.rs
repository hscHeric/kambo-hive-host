use kambo_hive::host::{result_aggregator::ResultAggregator, task_manager::TaskManager};
use kambo_hive::utils::init_logger;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::{env, fs, sync::Arc};
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
        eprintln!("Uso: {} <bind_addr:port> <graphs_path>", args[0]);
        return Ok(());
    }

    let addr = &args[1];
    let graphs_path = &args[2];

    let task_manager = Arc::new(Mutex::new(TaskManager::new()));
    let result_aggregator = Arc::new(Mutex::new(ResultAggregator::new()));

    // Configuração do Algoritmo Genético
    let ga_config = GAConfig {
        trials: 10,
        max_stagnant: 100,
        generations: 1000,
        tournament_size: 2,
        crossover_probability: 0.9,
        pop_size: None, // `None` para deixar o AG calcular
    };
    let ag_config_str = serde_json::to_string(&ga_config)?;

    // Ler a pasta de grafos e criar as tasks
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
    drop(tm); // Liberar o lock

    // Iniciar o servidor
    if let Err(e) =
        kambo_hive::host::server::start_server(addr, task_manager, result_aggregator).await
    {
        error!("Erro no servidor: {}", e);
    }

    Ok(())
}
