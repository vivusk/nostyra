//! The fake Drive's storage side: `q` matching, file / folder records, uploads
//! stored as new revisions, recursive delete, multipart parsing.

use serde_json::{json, Value};

use super::google_drive_fake_server::{Node, State};

impl State {
    /// The subset of `q` our provider writes.
    pub(super) fn list(&self, q: &str) -> Value {
        let quoted = |after: &str| {
            q.split(after).nth(1).map(|rest| {
                let mut value = String::new();
                let mut chars = rest.chars();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => value.extend(chars.next()),
                        '\'' => break,
                        c => value.push(c),
                    }
                }
                value
            })
        };
        let parent = q
            .split("' in parents")
            .next()
            .filter(|_| q.contains("in parents"))
            .map(|head| head.rsplit('\'').next().unwrap().to_string());
        let name = quoted("name = '");
        let marker = quoted("value='");
        let folders = q
            .contains("mimeType = ")
            .then_some(true)
            .or(q.contains("mimeType != ").then_some(false));
        let mut hits: Vec<Value> = self
            .order
            .iter()
            .filter_map(|id| Some((id, self.nodes.get(id)?)))
            .filter(|(_, n)| parent.is_none() || n.parent == parent)
            .filter(|(_, n)| name.as_ref().is_none_or(|want| &n.name == want))
            .filter(|(_, n)| marker.is_none() || n.marker == marker)
            .filter(|(_, n)| folders.is_none_or(|f| n.folder == f))
            .map(|(id, n)| self.meta(id, n))
            .collect();
        if q.contains("mimeType != ") {
            hits.reverse(); // modifiedTime desc ≈ newest created first
        }
        json!({ "files": hits })
    }

    fn meta(&self, id: &str, node: &Node) -> Value {
        let mime = if node.folder {
            "application/vnd.google-apps.folder"
        } else {
            "application/octet-stream"
        };
        let mut meta = json!({ "id": id, "name": node.name, "mimeType": mime, "headRevisionId": format!("rev{}", node.rev) });
        if !node.folder {
            meta["size"] = json!(node.data.len().to_string());
        }
        meta
    }

    pub(super) fn create(&mut self, meta: &Value, data: Vec<u8>, folder: bool) -> Value {
        self.next += 1;
        let id = format!("id{}", self.next);
        let node = Node {
            name: meta["name"].as_str().unwrap().to_string(),
            parent: meta["parents"][0].as_str().map(str::to_string),
            folder,
            marker: meta["appProperties"]["nostyra"]
                .as_str()
                .map(str::to_string),
            data,
            rev: self.next,
        };
        self.order.push(id.clone());
        self.nodes.insert(id.clone(), node);
        self.meta(&id, &self.nodes[&id])
    }

    pub(super) fn store(&mut self, file: Option<String>, meta: &Value, data: Vec<u8>) -> Value {
        let Some(id) = file else {
            return self.create(meta, data, false);
        };
        self.next += 1;
        let node = self.nodes.get_mut(&id).unwrap();
        node.data = data;
        node.rev = self.next;
        let node = node.clone();
        self.meta(&id, &node)
    }

    pub(super) fn remove_tree(&mut self, id: &str) {
        let children: Vec<String> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.parent.as_deref() == Some(id))
            .map(|(child, _)| child.clone())
            .collect();
        children.iter().for_each(|child| self.remove_tree(child));
        self.nodes.remove(id);
    }
}

pub(super) fn split_multipart(body: &[u8], boundary: &str) -> (Value, Vec<u8>) {
    let head_end = find(body, b"\r\n\r\n").unwrap() + 4;
    let meta_end = head_end + find(&body[head_end..], b"\r\n--").unwrap();
    let meta = serde_json::from_slice(&body[head_end..meta_end]).unwrap();
    let rest = &body[meta_end..];
    let media_start = find(rest, b"\r\n\r\n").unwrap() + 4;
    let tail = format!("\r\n--{boundary}--\r\n");
    (meta, rest[media_start..rest.len() - tail.len()].to_vec())
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
