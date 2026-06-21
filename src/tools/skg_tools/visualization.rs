// ==========================================
// 📊 SKG VISUALIZATION TOOLS — Native Skelegent Implementations
// ==========================================

use plotters::prelude::*;
use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;
use std::path::Path;

#[derive(serde::Deserialize, Debug, schemars::JsonSchema)]
pub struct SeriesData {
    name: String,
    x: Vec<f64>,
    y: Vec<f64>,
}

// ── generate_graph ─────────────────────────────────────────────────────────────

#[skg_tool(
    name = "generate_graph",
    description = "Generates a PNG graph/chart (line or scatter) from structured numeric data and saves it to a specified output_path. Provide 'chart_type' ('line' or 'scatter'), 'title', 'output_path' (e.g. 'chart.png'), and 'series' (array of objects with 'name', 'x', 'y' arrays)."
)]
pub async fn generate_graph(
    chart_type: String,
    title: String,
    x_label: Option<String>,
    y_label: Option<String>,
    output_path: String,
    series: Vec<SeriesData>,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    if series.is_empty() {
        return Err(ToolError::ExecutionFailed(
            "No data series provided.".to_string(),
        ));
    }

    let mut min_x = f64::MAX;
    let mut max_x = f64::MIN;
    let mut min_y = f64::MAX;
    let mut max_y = f64::MIN;

    for s in &series {
        for &x in &s.x {
            if x < min_x {
                min_x = x;
            }
            if x > max_x {
                max_x = x;
            }
        }
        for &y in &s.y {
            if y < min_y {
                min_y = y;
            }
            if y > max_y {
                max_y = y;
            }
        }
    }

    if min_x > max_x || min_y > max_y {
        min_x = 0.0;
        max_x = 1.0;
        min_y = 0.0;
        max_y = 1.0;
    } else {
        let range_x_pad = (max_x - min_x) * 0.1;
        let range_y_pad = (max_y - min_y) * 0.1;
        min_x -= range_x_pad.max(0.01);
        max_x += range_x_pad.max(0.01);
        min_y -= range_y_pad.max(0.01);
        max_y += range_y_pad.max(0.01);
    }

    let out_path = Path::new(&output_path);

    // Proposed Improvement: Automatically create the parent directory if it's missing
    if let Some(parent) = out_path.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent).map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to create output directory: {}", e))
        })?;
    }

    let root = BitMapBackend::new(&out_path, (800, 600)).into_drawing_area();
    root.fill(&WHITE)
        .map_err(|e| ToolError::ExecutionFailed(format!("Drawing error: {}", e)))?;

    let mut chart = ChartBuilder::on(&root)
        .caption(&title, ("sans-serif", 40).into_font())
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(50)
        .build_cartesian_2d(min_x..max_x, min_y..max_y)
        .map_err(|e| ToolError::ExecutionFailed(format!("Chart creation error: {}", e)))?;

    let mut mesh = chart.configure_mesh();
    if let Some(ref lbl) = x_label {
        mesh.x_desc(lbl);
    }
    if let Some(ref lbl) = y_label {
        mesh.y_desc(lbl);
    }
    mesh.draw()
        .map_err(|e| ToolError::ExecutionFailed(format!("Mesh draw error: {}", e)))?;

    let colors = [&RED, &BLUE, &GREEN, &MAGENTA, &CYAN, &YELLOW];

    for (i, s) in series.iter().enumerate() {
        if s.x.len() != s.y.len() {
            return Err(ToolError::ExecutionFailed(format!(
                "Series '{}' x and y arrays must have the same length",
                s.name
            )));
        }

        let color = *colors[i % colors.len()];
        let data_points: Vec<(f64, f64)> = s.x.iter().copied().zip(s.y.iter().copied()).collect();

        if chart_type.to_lowercase() == "scatter" {
            chart
                .draw_series(
                    data_points
                        .iter()
                        .map(|point| Circle::new(*point, 5, color.filled())),
                )
                .map_err(|e| ToolError::ExecutionFailed(format!("Draw error: {}", e)))?
                .label(&s.name)
                .legend(move |(x, y)| Circle::new((x, y), 5, color.filled()));
        } else {
            chart
                .draw_series(LineSeries::new(data_points, color))
                .map_err(|e| ToolError::ExecutionFailed(format!("Draw error: {}", e)))?
                .label(&s.name)
                .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], color));
        }
    }

    chart
        .configure_series_labels()
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .draw()
        .map_err(|e| ToolError::ExecutionFailed(format!("Legend draw error: {}", e)))?;

    root.present()
        .map_err(|e| ToolError::ExecutionFailed(format!("File save error: {}", e)))?;

    let abs_path = std::fs::canonicalize(out_path).unwrap_or_else(|_| out_path.to_path_buf());

    Ok(serde_json::Value::String(format!(
        "Successfully generated chart and saved to: {}",
        abs_path.display()
    )))
}
