use std::{collections::HashMap, net::SocketAddr, str::FromStr, time::Instant};

use super::super::action::Action;
use super::{Component, PageComponent, PageComponentBindings};
use crate::{
    database::config::DatabaseConfig, rpc::ConnectionInfo, state::State,
    tui::config::parse_key_event,
};
use color_eyre::Result;
use knowls::util::oneof::OneOf;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
    Frame,
};

#[derive(Debug, Clone)]
struct ComponentConnectionInfo {
    pub addr: SocketAddr,
    pub established: Instant,
}

#[derive(Debug, Clone)]
pub struct ConnectionsComponent {
    connections: Vec<ComponentConnectionInfo>,
    bindings: PageComponentBindings,
}

impl From<&State> for ConnectionsComponent {
    fn from(value: &State) -> Self {
        Self {
            connections: value
                .connections
                .iter()
                .fold(vec![], |mut acc, (addr, info)| {
                    acc.push(ComponentConnectionInfo {
                        addr: SocketAddr::from_str(addr).unwrap(),
                        established: info.established,
                    });
                    acc
                }),
            bindings: HashMap::new(),
        }
    }
}

impl PageComponent for ConnectionsComponent {
    fn id(&self) -> super::ComponentId {
        "connections".into()
    }
    fn selection_keys(&self) -> Vec<crossterm::event::KeyEvent> {
        vec![parse_key_event("c").unwrap()]
    }
    fn bindings(&self) -> &super::PageComponentBindings {
        &self.bindings
    }
    fn handle_action(&mut self, action: &super::PageComponentAction) -> Result<Option<Action>> {
        Ok(None)
    }
}

impl ConnectionsComponent {
    fn render_connection(info: &ComponentConnectionInfo, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .title(format!("● {}", info.addr.to_string()))
            .title_style(Style::new().green());

        let lines = vec![Line::raw(format!("Established: {:?}", info.established))];

        Paragraph::new(lines).block(block).render(area, buf);
    }
}

impl Component for ConnectionsComponent {
    fn update(&mut self, _state: &State, action: Action) -> Result<Option<Action>> {
        match action {
            // Action::Tick => {
            //     if let OneOf::Left(ref mut throbber) = self.healthy {
            //         throbber.calc_next();
            //     }
            // }
            _ => {}
        };
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let constraints = (0..self.connections.len() + 1)
            .into_iter()
            .fold(vec![], |mut acc, _| {
                acc.push(Constraint::Min(0));
                acc
            });
        let chunks = Layout::vertical(constraints)
            .flex(ratatui::layout::Flex::Start)
            .split(area);

        let throbber_chunk = chunks.last().take().unwrap();

        for (i, connection) in self.connections.iter_mut().enumerate() {
            Self::render_connection(connection, chunks[i], frame.buffer_mut());
        }

        let simple = throbber_widgets_tui::Throbber::default();
        frame.render_widget(simple, *throbber_chunk);
        Ok(())
    }
}
