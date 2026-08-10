use waml::model::Node;
use waml::solve::Rect;

use crate::{NodeVisualKind, StructuralVisualPolicy};

const ACTOR_FIGURE_WIDTH: f64 = 48.0;
const ACTOR_FIGURE_HEIGHT: f64 = 72.0;
const ACTOR_TITLE_GAP: f64 = 8.0;
const ELLIPSE_MIN_WIDTH: f64 = 140.0;
const ELLIPSE_MIN_HEIGHT: f64 = 72.0;
const ELLIPSE_PADDING_X: f64 = 36.0;
const ELLIPSE_PADDING_Y: f64 = 20.0;
const MAX_TITLE_WIDTH: f64 = 168.0;
const MAX_TITLE_LINES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub from: Point,
    pub to: Point,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum MeasuredNodeGeometry {
    #[default]
    ClassCard,
    Actor(ActorGeometry),
    UseCase(UseCaseGeometry),
    Note,
    Package,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActorGeometry {
    pub bounds: Rect,
    pub head_center: Point,
    pub head_radius: f64,
    pub body: Segment,
    pub arms: [Segment; 2],
    pub legs: [Segment; 2],
    pub title_bounds: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UseCaseGeometry {
    pub bounds: Rect,
    pub title_bounds: Rect,
    pub title_lines: Vec<String>,
}

pub trait TextMeasurer {
    fn measure(&self, text: &str) -> (f64, f64);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MonoTextMeasurer;

impl TextMeasurer for MonoTextMeasurer {
    fn measure(&self, text: &str) -> (f64, f64) {
        (text.chars().count() as f64 * 7.2, 16.0)
    }
}

impl MeasuredNodeGeometry {
    pub fn bounds(&self) -> Option<Rect> {
        match self {
            Self::Actor(geometry) => Some(geometry.bounds),
            Self::UseCase(geometry) => Some(geometry.bounds),
            Self::ClassCard | Self::Note | Self::Package => None,
        }
    }

    pub fn translated(&self, x: f64, y: f64) -> Self {
        match self {
            Self::Actor(geometry) => Self::Actor(geometry.translated(x, y)),
            Self::UseCase(geometry) => Self::UseCase(geometry.translated(x, y)),
            other => other.clone(),
        }
    }
}

impl ActorGeometry {
    fn translated(&self, x: f64, y: f64) -> Self {
        let point = |point: Point| Point {
            x: point.x + x,
            y: point.y + y,
        };
        let segment = |segment: Segment| Segment {
            from: point(segment.from),
            to: point(segment.to),
        };
        Self {
            bounds: translate_rect(self.bounds, x, y),
            head_center: point(self.head_center),
            head_radius: self.head_radius,
            body: segment(self.body),
            arms: self.arms.map(segment),
            legs: self.legs.map(segment),
            title_bounds: translate_rect(self.title_bounds, x, y),
        }
    }
}

impl UseCaseGeometry {
    fn translated(&self, x: f64, y: f64) -> Self {
        Self {
            bounds: translate_rect(self.bounds, x, y),
            title_bounds: translate_rect(self.title_bounds, x, y),
            title_lines: self.title_lines.clone(),
        }
    }
}

pub fn measure_node(
    policy: StructuralVisualPolicy,
    node: &Node,
    text: &dyn TextMeasurer,
) -> MeasuredNodeGeometry {
    let title = node.concept.title.as_deref().unwrap_or(node.key.as_str());
    match policy.node_kind(&node.ty) {
        NodeVisualKind::Actor => MeasuredNodeGeometry::Actor(measure_actor(title, text)),
        NodeVisualKind::UseCase => MeasuredNodeGeometry::UseCase(measure_use_case(title, text)),
        NodeVisualKind::Note => MeasuredNodeGeometry::Note,
        NodeVisualKind::Package => MeasuredNodeGeometry::Package,
        NodeVisualKind::ClassCard => MeasuredNodeGeometry::ClassCard,
    }
}

fn measure_actor(title: &str, text: &dyn TextMeasurer) -> ActorGeometry {
    let (title_width, title_height) = text.measure(title);
    let width = ACTOR_FIGURE_WIDTH.max(title_width);
    let figure_left = (width - ACTOR_FIGURE_WIDTH) / 2.0;
    let center_x = width / 2.0;
    let head_center = Point {
        x: center_x,
        y: 12.0,
    };
    let body = Segment {
        from: Point {
            x: center_x,
            y: 24.0,
        },
        to: Point {
            x: center_x,
            y: 50.0,
        },
    };
    ActorGeometry {
        bounds: Rect {
            x: 0.0,
            y: 0.0,
            w: width,
            h: ACTOR_FIGURE_HEIGHT + ACTOR_TITLE_GAP + title_height,
        },
        head_center,
        head_radius: 10.0,
        body,
        arms: [
            Segment {
                from: Point {
                    x: center_x,
                    y: 32.0,
                },
                to: Point {
                    x: figure_left,
                    y: 44.0,
                },
            },
            Segment {
                from: Point {
                    x: center_x,
                    y: 32.0,
                },
                to: Point {
                    x: figure_left + ACTOR_FIGURE_WIDTH,
                    y: 44.0,
                },
            },
        ],
        legs: [
            Segment {
                from: Point {
                    x: center_x,
                    y: 50.0,
                },
                to: Point {
                    x: figure_left + 4.0,
                    y: ACTOR_FIGURE_HEIGHT,
                },
            },
            Segment {
                from: Point {
                    x: center_x,
                    y: 50.0,
                },
                to: Point {
                    x: figure_left + ACTOR_FIGURE_WIDTH - 4.0,
                    y: ACTOR_FIGURE_HEIGHT,
                },
            },
        ],
        title_bounds: Rect {
            x: (width - title_width) / 2.0,
            y: ACTOR_FIGURE_HEIGHT + ACTOR_TITLE_GAP,
            w: title_width,
            h: title_height,
        },
    }
}

fn measure_use_case(title: &str, text: &dyn TextMeasurer) -> UseCaseGeometry {
    let mut lines = wrap_title(title, text);
    if lines.len() > MAX_TITLE_LINES {
        lines.truncate(MAX_TITLE_LINES);
        let last = lines.last_mut().unwrap();
        ellipsize(last, text, MAX_TITLE_WIDTH);
    }
    for line in &mut lines {
        if text.measure(line).0 > MAX_TITLE_WIDTH {
            ellipsize(line, text, MAX_TITLE_WIDTH);
        }
    }
    let line_height = text.measure("Mg").1;
    let title_width = lines
        .iter()
        .map(|line| text.measure(line).0)
        .fold(0.0, f64::max);
    let title_height = line_height * lines.len().max(1) as f64;
    let width = ELLIPSE_MIN_WIDTH.max(title_width + ELLIPSE_PADDING_X * 2.0);
    let height = ELLIPSE_MIN_HEIGHT.max(title_height + ELLIPSE_PADDING_Y * 2.0);
    UseCaseGeometry {
        bounds: Rect {
            x: 0.0,
            y: 0.0,
            w: width,
            h: height,
        },
        title_bounds: Rect {
            x: (width - title_width) / 2.0,
            y: (height - title_height) / 2.0,
            w: title_width,
            h: title_height,
        },
        title_lines: lines,
    }
}

fn wrap_title(title: &str, text: &dyn TextMeasurer) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in title.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if !current.is_empty() && text.measure(&candidate).0 > MAX_TITLE_WIDTH {
            lines.push(current);
            current = word.to_string();
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn ellipsize(line: &mut String, text: &dyn TextMeasurer, max_width: f64) {
    while !line.is_empty() && text.measure(&format!("{line}…")).0 > max_width {
        line.pop();
    }
    line.push('…');
}

fn translate_rect(rect: Rect, x: f64, y: f64) -> Rect {
    Rect {
        x: rect.x + x,
        y: rect.y + y,
        ..rect
    }
}
