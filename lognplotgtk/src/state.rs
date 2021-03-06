use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use crate::session;
use lognplot::chart::{Chart, Curve, CurveData};
use lognplot::geometry::Size;
use lognplot::render::{x_pixel_to_domain, x_pixels_to_domain};
use lognplot::time::TimeStamp;
use lognplot::tsdb::{Aggregation, Observation, Sample, SampleMetrics, TsDbHandle};

/// Struct with some GUI state in it which will be shown in the GUI.
pub struct GuiState {
    pub chart: Chart,
    gui_start_instant: Instant,
    pub db: TsDbHandle,
    // TODO:
    color_wheel: Vec<String>,
    color_index: usize,
    tailing: Option<f64>,

    drag: Option<(f64, f64)>,
}

fn new_chart() -> Chart {
    let mut chart = Chart::default();
    chart.set_xlabel("Time");
    chart.set_ylabel("Value");
    chart.set_title("W00tie");
    chart
}

impl GuiState {
    pub fn new(db: TsDbHandle) -> Self {
        let chart = new_chart();
        let color_wheel = vec!["blue".to_string(), "red".to_string(), "green".to_string()];
        GuiState {
            chart,
            gui_start_instant: Instant::now(),
            db,
            color_wheel,
            color_index: 0,
            tailing: None,
            drag: None,
        }
    }

    /// This is cool stuff, log metrics about render time for example to database itself :)
    pub fn log_meta_metric(&self, name: &str, timestamp: Instant, value: f64) {
        let elapsed = timestamp.duration_since(self.gui_start_instant);
        let elapsed_seconds: f64 = elapsed.as_secs_f64();
        let timestamp = TimeStamp::new(elapsed_seconds);
        let observation = Observation::new(timestamp, Sample::new(value));
        self.db.add_value(name, observation);
    }

    pub fn into_handle(self) -> GuiStateHandle {
        Rc::new(RefCell::new(self))
    }

    #[cfg(feature = "export-hdf5")]
    pub fn save(&self, filename: &Path) -> Result<(), String> {
        info!("Save data to {:?}", filename);
        super::io::export_data(self.db.clone(), filename).map_err(|e| e.to_string())
    }

    #[cfg(not(feature = "export-hdf5"))]
    pub fn save(&self, filename: &Path) -> Result<(), String> {
        let msg = format!(
            "Not saving to {:?}, since export-hdf5 feature is not enabled.",
            filename
        );
        Err(msg)
    }

    pub fn save_session(&self, filename: &Path) -> std::io::Result<()> {
        let mut s = session::Session::new();
        s.add_item((&self.chart).into());
        let f = std::fs::File::create(filename)?;
        serde_json::to_writer(f, &s)?;
        Ok(())
    }

    pub fn load_session(&mut self, filename: &Path) -> std::io::Result<()> {
        let f = std::fs::File::open(filename)?;
        let s: session::Session = serde_json::from_reader(f)?;
        if let Some(session::DashBoardItem::Graph { curves }) = s.first() {
            self.clear_curves();
            for curve in curves {
                self.add_curve(curve);
            }
        }
        Ok(())
    }

    pub fn get_signal_summary(&self, name: &str) -> Option<Aggregation<Sample, SampleMetrics>> {
        self.db.summary(name, None)
    }

    pub fn add_curve(&mut self, name: &str) {
        // self.chart.add_curve(Curve::new());
        if !self.chart.has_signal(name) {
            let tsdb_data = CurveData::trace(name, self.db.clone());
            let color = self.next_color();
            let curve2 = Curve::new(tsdb_data, &color);

            self.chart.add_curve(curve2);
            self.chart.autoscale();
        } else {
            info!("Signal {} is already shown", name);
        }
    }

    pub fn next_color(&mut self) -> String {
        let color = self.color_wheel[self.color_index].clone();
        self.color_index += 1;
        if self.color_index >= self.color_wheel.len() {
            self.color_index = 0;
        }
        color
    }

    /// Initial drag action of the mouse
    pub fn start_drag(&mut self, x: f64, y: f64) {
        debug!("Drag start! {}, {} ", x, y);
        self.disable_tailing();
        self.drag = Some((x, y));
    }

    /// Update drag of the mouse
    pub fn move_drag(&mut self, size: Size, x: f64, y: f64) {
        self.disable_tailing();
        if let Some((prev_x, prev_y)) = self.drag {
            let dx = x - prev_x;
            let dy = y - prev_y;
            self.do_drag(size, dx, dy);
        }
        self.drag = Some((x, y));
    }

    /// Drag the plot by the given amount.
    fn do_drag(&mut self, size: Size, dx: f64, dy: f64) {
        debug!("Drag! {}, {} ", dx, dy);

        let amount = x_pixels_to_domain(size, &self.chart.x_axis, dx);

        self.chart.pan_horizontal_absolute(-amount);
        // TODO: pan vertical as well?
        // TODO: idea: auto fit vertically?
        self.chart.fit_y_axis();
        // self.chart.pan_vertical(dy* 0.001);
    }

    pub fn zoom_fit(&mut self) {
        debug!("Autoscale!");
        self.disable_tailing();
        self.chart.autoscale();
    }

    pub fn clear_curves(&mut self) {
        debug!("Kill all signals!");
        self.disable_tailing();
        self.chart.clear_curves();
    }

    pub fn pan_left(&mut self) {
        debug!("pan left!");
        self.disable_tailing();
        self.chart.pan_horizontal_relative(-0.1);
        self.chart.fit_y_axis();
    }

    pub fn pan_right(&mut self) {
        debug!("Pan right!");
        self.disable_tailing();
        self.chart.pan_horizontal_relative(0.1);
        self.chart.fit_y_axis();
    }

    pub fn pan_up(&mut self) {
        debug!("pan up!");
        self.disable_tailing();
        self.chart.pan_vertical(-0.1);
    }

    pub fn pan_down(&mut self) {
        debug!("pan down!");
        self.disable_tailing();
        self.chart.pan_vertical(0.1);
    }

    pub fn zoom_in_vertical(&mut self) {
        debug!("Zoom in vertical");
        self.zoom_vertical(0.1);
    }

    pub fn zoom_out_vertical(&mut self) {
        debug!("Zoom out vertical");
        self.zoom_vertical(-0.1);
    }

    fn zoom_vertical(&mut self, amount: f64) {
        self.disable_tailing();
        self.chart.zoom_vertical(amount);
    }

    pub fn zoom_in_horizontal(&mut self, around: Option<(f64, Size)>) {
        debug!("Zoom in horizontal");
        self.zoom_horizontal(-0.1, around);
    }

    pub fn zoom_out_horizontal(&mut self, around: Option<(f64, Size)>) {
        debug!("Zoom out horizontal");
        self.zoom_horizontal(0.1, around);
    }

    fn zoom_horizontal(&mut self, amount: f64, around: Option<(f64, Size)>) {
        let around = around.map(|p| {
            let (pixel, size) = p;
            x_pixel_to_domain(pixel, &self.chart.x_axis, size)
        });
        self.disable_tailing();
        self.chart.zoom_horizontal(amount, around);
        self.chart.fit_y_axis();
    }

    pub fn set_cursor(&mut self, loc: Option<(f64, Size)>) {
        if let Some((pixel, size)) = loc {
            let timestamp = x_pixel_to_domain(pixel, &self.chart.x_axis, size);
            let timestamp = TimeStamp::new(timestamp);
            self.chart.cursor = Some(timestamp);
        } else {
            self.chart.cursor = None;
        }
    }

    pub fn zoom_to_last(&mut self, tail_duration: f64) {
        self.chart.zoom_to_last(tail_duration);
        self.chart.fit_y_axis();
    }

    pub fn enable_tailing(&mut self, tail_duration: f64) {
        self.tailing = Some(tail_duration);
    }

    pub fn disable_tailing(&mut self) {
        self.tailing = None;
    }

    pub fn do_tailing(&mut self) -> bool {
        if let Some(x) = self.tailing {
            self.zoom_to_last(x);
            true
        } else {
            false
        }
    }
}

pub type GuiStateHandle = Rc<RefCell<GuiState>>;

impl std::fmt::Display for GuiState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Db: {}", self.db)
    }
}
