use aws_config;
use aws_config::BehaviorVersion;
use aws_sdk_cloudwatch::{types::Statistic, Client as CloudWatchClient};
use aws_sdk_neptune::Client as NeptuneClient;
use std::time::SystemTime;

use chrono::{self};


use crate::utils::AppError;

pub async fn list_neptune() -> Result<(), AppError> {
    let config = aws_config::load_defaults(BehaviorVersion::v2024_03_28()).await;
    let client = NeptuneClient::new(&config);
    let cloudwatch_client = CloudWatchClient::new(&config);
    // Describe Neptune clusters
    let clusters = match client.describe_db_clusters().send().await {
        Ok(resp) => resp,
        Err(e) => {
            let err_str: String = format!("Failed to describe clusters: {}", e);
            return Err(AppError::CommandFailed(err_str));
        }
    };

    println!("{}", " ");
    let title = "NEPTUNE CLUSTER INFORMATION";
    let separator = "=".repeat(63);
    let lines = "\x1b[1m=\x1b[0m".repeat(55);
    println!(
        "{:^1$}",
        format!("\x1b[1m{}\x1b[0m", title),
        separator.len()
    );
    println!("{}", lines);
    println!("{}", " ");
    for cluster in clusters.db_clusters() {
        // Get CPU Utilization from CloudWatch
        let metric_name = "CPUUtilization";
        let namespace = "AWS/Neptune";

        let chrono_start_time = chrono::Utc::now() - chrono::Duration::minutes(5);
        let chrono_end_time = chrono::Utc::now();

        let start_time =
            aws_sdk_ec2::primitives::DateTime::from(SystemTime::from(chrono_start_time));
        let end_time = aws_sdk_ec2::primitives::DateTime::from(SystemTime::from(chrono_end_time));

        let cpu_util_resp = cloudwatch_client
            .get_metric_statistics()
            .namespace(namespace)
            .metric_name(metric_name)
            .start_time(start_time) // Last 1 hour
            .end_time(end_time)
            .period(300) // 5 minutes periods
            .statistics(Statistic::Average)
            .dimensions(
                aws_sdk_cloudwatch::types::Dimension::builder()
                    .name("DBClusterIdentifier")
                    .value(cluster.db_cluster_identifier().unwrap_or_default())
                    .build(),
            )
            .send()
            .await;

        let mut cpu_util: Option<(String, f64)> = None;
        if let Ok(stats) = cpu_util_resp {
            for point in stats.datapoints() {
                match (point.timestamp(), point.average()) {
                    (Some(timestamp), Some(average)) => {
                        let timestamp_str = timestamp.to_string();
                        match chrono::DateTime::parse_from_rfc3339(&timestamp_str) {
                            Ok(datetime) => {
                                let formatted_timestamp =
                                    datetime.format("%I:%M%p %d/%m/%Y").to_string();
                                cpu_util = Some((formatted_timestamp, average));
                            }
                            Err(e) => {
                                eprintln!("Failed to parse timestamp: {}", e);
                            }
                        }
                    }
                    _ => {
                        println!("Missing data");
                    }
                }
            }
        } else {
            eprintln!("Failed to get CPU utilization metrics.");
        }
        let cluster_name = cluster.db_cluster_identifier().unwrap_or_default();
        let status = cluster.status().unwrap_or_default();
        let endpoint = cluster.endpoint().unwrap_or_default();

        let instance_count = cluster.db_cluster_members().len();

        // Construct the AWS console link for the cluster
        let cluster_link = format!(
            "https://console.aws.amazon.com/neptune/home?region={}#database:id={};is-cluster=true",
            config.region().unwrap().as_ref(),
            cluster.db_cluster_identifier().unwrap_or_default()
        );
        println!("\x1b[1m{} {}\x1b[0m", "Cluster:", cluster_name);
        println!("{}", "\x1b[1m-\x1b[0m".repeat(55));
        println!("\x1b[1m{:<16}\x1b[0m {}", "Instance Count:", instance_count);
        println!("\x1b[1m{:<16}\x1b[0m {}", "Status:", status);
        println!(
            "\x1b[1m{:<16}\x1b[0m {}",
            "CPU Utilisation:",
            match cpu_util {
                Some((timestamp, average)) => format!("{:.2}% at {}", average, timestamp),
                None => "N/A".to_string(),
            }
        );
        println!("\x1b[1m{:<16}\x1b[0m {}", "Endpoint:", endpoint);
        println!("\x1b[1m{:<16}\x1b[0m {}", "Cluster Link:", cluster_link);
        println!("{}", " ");
    }
    return Ok(());
}
