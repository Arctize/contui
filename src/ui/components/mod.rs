//! UI components

pub mod container_list;
pub mod detail_panel;
pub mod detail_viewer;
pub mod exec_viewer;
pub mod image_detail_viewer;
pub mod image_list;
pub mod log_viewer;
pub mod network_list;
pub mod stats_viewer;
pub mod view_search;
pub mod volume_list;

pub use container_list::ContainerListWidget;
pub use detail_panel::{ContainerDetailPanel, SplitLayout};
pub use detail_viewer::render_detail_viewer;
pub use exec_viewer::{render_exec_panel, EXEC_PANEL_HEIGHT};
pub use image_detail_viewer::render_image_detail_viewer;
pub use image_list::ImageListWidget;
pub use network_list::NetworkListWidget;
pub use stats_viewer::{render_stats_panel, STATS_PANEL_HEIGHT};
pub use volume_list::VolumeListWidget;
