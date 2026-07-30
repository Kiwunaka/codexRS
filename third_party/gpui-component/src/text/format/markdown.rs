use std::ops::Range;

use gpui::{HighlightStyle, SharedString};
use markdown::{
    ParseOptions,
    mdast::{self, Node},
};

use crate::{
    highlighter::HighlightTheme,
    text::{
        TextViewStyle,
        node::{
            self, CodeBlock, ImageNode, InlineNode, LinkMark, NodeContext, Paragraph, Span, Table,
            TableRow, TextMark,
        },
    },
};

/// Parse Markdown into a tree of nodes.
pub(crate) fn parse(
    raw: &str,
    style: &TextViewStyle,
    cx: &mut NodeContext,
    highlight_theme: &HighlightTheme,
) -> Result<node::Node, SharedString> {
    markdown::to_mdast(&raw, &ParseOptions::gfm())
        .map(|n| ast_to_node(n, style, cx, highlight_theme))
        .map_err(|e| e.to_string().into())
}

fn source_value_range(node: &mdast::Node, value: &str, cx: &NodeContext) -> Option<Range<usize>> {
    let source = cx.source.as_deref()?;
    let position = node.position()?;
    if position.start.offset > position.end.offset
        || position.end.offset > source.len()
        || !source.is_char_boundary(position.start.offset)
        || !source.is_char_boundary(position.end.offset)
    {
        return None;
    }
    let node_source = &source[position.start.offset..position.end.offset];
    if node_source == value {
        return Some(position.start.offset..position.end.offset);
    }
    let relative_start = node_source.find(value)?;
    let start = position.start.offset + relative_start;
    let end = start + value.len();
    (source.is_char_boundary(start) && source.is_char_boundary(end)).then_some(start..end)
}

fn source_highlight_styles(
    node: &mdast::Node,
    value: &str,
    cx: &NodeContext,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let Some(value_range) = source_value_range(node, value, cx) else {
        return Vec::new();
    };
    cx.source_highlights
        .iter()
        .filter_map(|highlight| {
            let start = highlight.range.start.max(value_range.start);
            let end = highlight.range.end.min(value_range.end);
            (start < end).then(|| {
                (
                    (start - value_range.start)..(end - value_range.start),
                    highlight.style,
                )
            })
        })
        .collect()
}

fn source_highlight_marks(
    node: &mdast::Node,
    value: &str,
    cx: &NodeContext,
) -> Vec<(Range<usize>, TextMark)> {
    source_highlight_styles(node, value, cx)
        .into_iter()
        .map(|(range, highlight)| {
            (
                range,
                TextMark {
                    highlight: Some(highlight),
                    ..Default::default()
                },
            )
        })
        .collect()
}

fn mark_paragraph(paragraph: &mut Paragraph, mark: TextMark) {
    for child in paragraph.children.iter_mut() {
        if !child.text.is_empty() {
            child.marks.push((0..child.text.len(), mark.clone()));
        }
    }
}

fn parse_table_row(table: &mut Table, node: &mdast::TableRow, cx: &mut NodeContext) {
    let mut row = TableRow::default();
    node.children.iter().for_each(|c| {
        match c {
            Node::TableCell(cell) => {
                parse_table_cell(&mut row, cell, cx);
            }
            _ => {}
        };
    });
    table.children.push(row);
}

fn parse_table_cell(row: &mut node::TableRow, node: &mdast::TableCell, cx: &mut NodeContext) {
    let mut paragraph = Paragraph::default();
    node.children.iter().for_each(|c| {
        parse_paragraph(&mut paragraph, c, cx);
    });
    let table_cell = node::TableCell {
        children: paragraph,
        ..Default::default()
    };
    row.children.push(table_cell);
}

fn parse_paragraph(paragraph: &mut Paragraph, node: &mdast::Node, cx: &mut NodeContext) -> String {
    let span = node.position().map(|pos| Span {
        start: pos.start.offset,
        end: pos.end.offset,
    });
    if let Some(span) = span {
        paragraph.set_span(span);
    }

    let mut text = String::new();

    match node {
        Node::Paragraph(val) => {
            val.children.iter().for_each(|c| {
                text.push_str(&parse_paragraph(paragraph, c, cx));
            });
        }
        Node::Text(val) => {
            text = val.value.clone();
            paragraph.push(
                InlineNode::new(&val.value).marks(source_highlight_marks(node, &val.value, cx)),
            )
        }
        Node::Emphasis(val) => {
            let mut child_paragraph = Paragraph::default();
            for child in val.children.iter() {
                text.push_str(&parse_paragraph(&mut child_paragraph, &child, cx));
            }
            mark_paragraph(&mut child_paragraph, TextMark::default().italic());
            paragraph.merge(child_paragraph);
        }
        Node::Strong(val) => {
            let mut child_paragraph = Paragraph::default();
            for child in val.children.iter() {
                text.push_str(&parse_paragraph(&mut child_paragraph, &child, cx));
            }
            mark_paragraph(&mut child_paragraph, TextMark::default().bold());
            paragraph.merge(child_paragraph);
        }
        Node::Delete(val) => {
            let mut child_paragraph = Paragraph::default();
            for child in val.children.iter() {
                text.push_str(&parse_paragraph(&mut child_paragraph, &child, cx));
            }
            mark_paragraph(&mut child_paragraph, TextMark::default().strikethrough());
            paragraph.merge(child_paragraph);
        }
        Node::InlineCode(val) => {
            text = val.value.clone();
            let mut marks = vec![(0..text.len(), TextMark::default().code())];
            marks.extend(source_highlight_marks(node, &text, cx));
            paragraph.push(InlineNode::new(&text).marks(marks));
        }
        Node::Link(val) => {
            let link_mark = Some(LinkMark {
                url: val.url.clone().into(),
                title: val.title.clone().map(|s| s.into()),
                ..Default::default()
            });

            let mut child_paragraph = Paragraph::default();
            for child in val.children.iter() {
                text.push_str(&parse_paragraph(&mut child_paragraph, &child, cx));
            }

            // FIXME: GPUI InteractiveText does not support inline images yet.
            // So here we push images to the paragraph directly.
            for child in child_paragraph.children.iter_mut() {
                if let Some(image) = child.image.as_mut() {
                    image.link = link_mark.clone();
                }

                child.marks.push((
                    0..child.text.len(),
                    TextMark {
                        link: link_mark.clone(),
                        ..Default::default()
                    },
                ));
            }

            paragraph.merge(child_paragraph);
        }
        Node::Image(raw) => {
            paragraph.push_image(ImageNode {
                url: raw.url.clone().into(),
                title: raw.title.clone().map(|t| t.into()),
                alt: Some(raw.alt.clone().into()),
                ..Default::default()
            });
        }
        Node::InlineMath(raw) => {
            text = raw.value.clone();
            let mut marks = vec![(0..text.len(), TextMark::default().code())];
            marks.extend(source_highlight_marks(node, &text, cx));
            paragraph.push(InlineNode::new(&text).marks(marks));
        }
        Node::MdxTextExpression(raw) => {
            text = raw.value.clone();
            paragraph.push(InlineNode::new(&text).marks(source_highlight_marks(node, &text, cx)));
        }
        Node::Html(val) => match super::html::parse(&val.value, cx) {
            Ok(el) => {
                if el.is_break() {
                    text = "\n".to_owned();
                    paragraph.push(InlineNode::new(&text));
                } else {
                    if cfg!(debug_assertions) {
                        tracing::warn!("unsupported inline html tag: {:#?}", el);
                    }
                }
            }
            Err(err) => {
                if cfg!(debug_assertions) {
                    tracing::warn!("failed parsing html: {:#?}", err);
                }

                text.push_str(&val.value);
            }
        },
        Node::FootnoteReference(foot) => {
            let prefix = format!("[{}]", foot.identifier);
            paragraph.push(InlineNode::new(&prefix).marks(vec![(
                0..prefix.len(),
                TextMark {
                    italic: true,
                    ..Default::default()
                },
            )]));
        }
        Node::LinkReference(link) => {
            let mut child_paragraph = Paragraph::default();
            let mut child_text = String::new();
            for child in link.children.iter() {
                child_text.push_str(&parse_paragraph(&mut child_paragraph, child, cx));
            }

            let link_mark = LinkMark {
                url: "".into(),
                title: link.label.clone().map(Into::into),
                identifier: Some(link.identifier.clone().into()),
            };

            mark_paragraph(
                &mut child_paragraph,
                TextMark {
                    link: Some(link_mark),
                    ..Default::default()
                },
            );
            paragraph.merge(child_paragraph);
        }
        _ => {
            if cfg!(debug_assertions) {
                tracing::warn!("unsupported inline node: {:#?}", node);
            }
        }
    }

    text
}

fn ast_to_node(
    value: mdast::Node,
    style: &TextViewStyle,
    cx: &mut NodeContext,
    highlight_theme: &HighlightTheme,
) -> node::Node {
    match value {
        Node::Root(val) => {
            let children = val
                .children
                .into_iter()
                .map(|c| ast_to_node(c, style, cx, highlight_theme))
                .collect();
            node::Node::Root { children }
        }
        Node::Paragraph(val) => {
            let mut paragraph = Paragraph::default();
            val.children.iter().for_each(|c| {
                parse_paragraph(&mut paragraph, c, cx);
            });

            node::Node::Paragraph(paragraph)
        }
        Node::Blockquote(val) => {
            let children = val
                .children
                .into_iter()
                .map(|c| ast_to_node(c, style, cx, highlight_theme))
                .collect();
            node::Node::Blockquote { children }
        }
        Node::List(list) => {
            let children = list
                .children
                .into_iter()
                .map(|c| ast_to_node(c, style, cx, highlight_theme))
                .collect();
            node::Node::List {
                ordered: list.ordered,
                children,
            }
        }
        Node::ListItem(val) => {
            let children = val
                .children
                .into_iter()
                .map(|c| ast_to_node(c, style, cx, highlight_theme))
                .collect();
            node::Node::ListItem {
                children,
                spread: val.spread,
                checked: val.checked,
            }
        }
        Node::Break(_) => node::Node::Break { html: false },
        Node::Code(raw) => {
            let source_highlights =
                source_highlight_styles(&Node::Code(raw.clone()), &raw.value, cx);
            node::Node::CodeBlock(CodeBlock::new(
                raw.value.into(),
                raw.lang.map(|s| s.into()),
                style,
                highlight_theme,
                source_highlights,
            ))
        }
        Node::Heading(val) => {
            let mut paragraph = Paragraph::default();
            val.children.iter().for_each(|c| {
                parse_paragraph(&mut paragraph, c, cx);
            });

            node::Node::Heading {
                level: val.depth,
                children: paragraph,
            }
        }
        Node::Math(val) => node::Node::CodeBlock(CodeBlock::new(
            val.value.into(),
            None,
            style,
            highlight_theme,
            Vec::new(),
        )),
        Node::Html(val) => match super::html::parse(&val.value, cx) {
            Ok(el) => el,
            Err(err) => {
                if cfg!(debug_assertions) {
                    tracing::warn!("error parsing html: {:#?}", err);
                }

                node::Node::Paragraph(Paragraph::new(val.value))
            }
        },
        Node::MdxFlowExpression(val) => node::Node::CodeBlock(CodeBlock::new(
            val.value.into(),
            Some("mdx".into()),
            style,
            highlight_theme,
            Vec::new(),
        )),
        Node::Yaml(val) => node::Node::CodeBlock(CodeBlock::new(
            val.value.into(),
            Some("yml".into()),
            style,
            highlight_theme,
            Vec::new(),
        )),
        Node::Toml(val) => node::Node::CodeBlock(CodeBlock::new(
            val.value.into(),
            Some("toml".into()),
            style,
            highlight_theme,
            Vec::new(),
        )),
        Node::MdxJsxTextElement(val) => {
            let mut paragraph = Paragraph::default();
            val.children.iter().for_each(|c| {
                parse_paragraph(&mut paragraph, c, cx);
            });
            node::Node::Paragraph(paragraph)
        }
        Node::MdxJsxFlowElement(val) => {
            let mut paragraph = Paragraph::default();
            val.children.iter().for_each(|c| {
                parse_paragraph(&mut paragraph, c, cx);
            });
            node::Node::Paragraph(paragraph)
        }
        Node::ThematicBreak(_) => node::Node::Divider,
        Node::Table(val) => {
            let mut table = Table::default();
            table.column_aligns = val
                .align
                .clone()
                .into_iter()
                .map(|align| align.into())
                .collect();
            val.children.iter().for_each(|c| {
                if let Node::TableRow(row) = c {
                    parse_table_row(&mut table, row, cx);
                }
            });

            node::Node::Table(table)
        }
        Node::FootnoteDefinition(def) => {
            let mut paragraph = Paragraph::default();
            let prefix = format!("[{}]: ", def.identifier);
            paragraph.push(InlineNode::new(&prefix).marks(vec![(
                0..prefix.len(),
                TextMark {
                    italic: true,
                    ..Default::default()
                },
            )]));

            def.children.iter().for_each(|c| {
                parse_paragraph(&mut paragraph, c, cx);
            });
            node::Node::Paragraph(paragraph)
        }
        Node::Definition(def) => {
            cx.add_ref(
                def.identifier.clone().into(),
                LinkMark {
                    url: def.url.clone().into(),
                    identifier: Some(def.identifier.clone().into()),
                    title: def.title.clone().map(Into::into),
                },
            );

            node::Node::Definition {
                identifier: def.identifier.clone().into(),
                url: def.url.clone().into(),
                title: def.title.clone().map(|s| s.into()),
            }
        }
        _ => {
            if cfg!(debug_assertions) {
                tracing::warn!("unsupported node: {:#?}", value);
            }
            node::Node::Unknown
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui::{HighlightStyle, hsla};

    use super::*;
    use crate::text::SourceHighlight;

    #[test]
    fn source_highlights_preserve_nested_markdown_marks() {
        let source = "First **Needle** and [needle](https://example.com) plus `needle`.";
        let ranges = source
            .match_indices("Needle")
            .chain(source.match_indices("needle"))
            .map(|(start, value)| start..start + value.len())
            .collect::<Vec<_>>();
        let style = HighlightStyle {
            background_color: Some(hsla(0.1, 0.8, 0.5, 0.6)),
            ..Default::default()
        };
        let mut cx = NodeContext {
            source: Some(source.to_owned().into()),
            source_highlights: ranges
                .into_iter()
                .map(|range| SourceHighlight::new(range, style))
                .collect(),
            ..Default::default()
        };

        let parsed = parse(
            source,
            &TextViewStyle::default(),
            &mut cx,
            &HighlightTheme::default_light(),
        )
        .expect("markdown should parse");
        let node::Node::Root { children } = parsed else {
            panic!("expected root");
        };
        let node::Node::Paragraph(paragraph) = &children[0] else {
            panic!("expected paragraph");
        };
        let highlighted = paragraph
            .children
            .iter()
            .filter(|child| child.marks.iter().any(|(_, mark)| mark.highlight.is_some()))
            .collect::<Vec<_>>();

        assert_eq!(highlighted.len(), 3);
        assert!(highlighted.iter().all(|child| {
            child.marks.iter().any(|(range, mark)| {
                *range == (0..child.text.len()) && mark.highlight == Some(style)
            })
        }));
        assert!(highlighted[0].marks.iter().any(|(_, mark)| mark.bold));
        assert!(
            highlighted[1]
                .marks
                .iter()
                .any(|(_, mark)| mark.link.is_some())
        );
        assert!(highlighted[2].marks.iter().any(|(_, mark)| mark.code));
    }
}
