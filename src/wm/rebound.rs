use smithay::utils::{Logical, Point, Size};
use std::time::{Duration, Instant};

const EDGE_MARGIN: i32 = 20;
const REBOUND_THRESHOLD: i32 = 30;
const REBOUND_TIMEOUT: Duration = Duration::from_millis(350);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rebound {
    None,
    OnEdge {
        edge: Edge,
        origin: Point<i32, Logical>,
        at: Instant,
    },
    Expired,
}

impl Rebound {
    pub fn new() -> Self {
        Self::None
    }

    pub fn on_motion(
        &mut self,
        pos: Point<i32, Logical>,
        screen: Size<i32, Logical>,
        now: Instant,
    ) -> Option<Edge> {
        let edge = detect_edge(pos, screen);

        match *self {
            Rebound::None => {
                if let Some(edge) = edge {
                    *self = Rebound::OnEdge {
                        edge,
                        origin: pos,
                        at: now,
                    };
                }
                None
            }
            Rebound::OnEdge {
                edge: armed_edge,
                origin,
                at,
            } => {
                if now.duration_since(at) > REBOUND_TIMEOUT {
                    *self = Rebound::Expired;
                    return None;
                }

                let dx = pos.x - origin.x;
                let dy = pos.y - origin.y;
                let inward_delta = match armed_edge {
                    Edge::Left => dx,
                    Edge::Right => -dx,
                    Edge::Bottom => -dy,
                    Edge::Top => dy,
                };

                if inward_delta > REBOUND_THRESHOLD {
                    *self = Rebound::None;
                    Some(armed_edge)
                } else {
                    None
                }
            }
            Rebound::Expired => {
                // When expired, only reset once user leaves the edge
                if edge.is_none() {
                    *self = Rebound::None;
                }
                None
            }
        }
    }

    pub fn reset(&mut self) {
        *self = Rebound::None;
    }
}

fn detect_edge(pos: Point<i32, Logical>, screen: Size<i32, Logical>) -> Option<Edge> {
    if pos.x <= EDGE_MARGIN {
        Some(Edge::Left)
    } else if pos.x >= screen.w - EDGE_MARGIN {
        Some(Edge::Right)
    } else if pos.y >= screen.h - EDGE_MARGIN {
        Some(Edge::Bottom)
    } else if pos.y <= EDGE_MARGIN {
        Some(Edge::Top)
    } else {
        None
    }
}
