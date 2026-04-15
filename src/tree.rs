/// A node in the connection/object tree sidebar.
pub struct TreeNode {
    pub label: String,
    pub expanded: bool,
    pub children: Vec<TreeNode>,
}

/// Flattened representation of a tree node for rendering.
pub struct FlatNode {
    pub label: String,
    pub depth: u16,
    pub expanded: bool,
    pub has_children: bool,
}

impl TreeNode {
    pub fn connection(name: &str, children: Vec<TreeNode>) -> Self {
        Self {
            label: name.to_string(),
            expanded: false,
            children,
        }
    }

    pub fn folder(name: &str, children: Vec<TreeNode>) -> Self {
        Self {
            label: name.to_string(),
            expanded: false,
            children,
        }
    }

    pub fn leaf(name: &str) -> Self {
        Self {
            label: name.to_string(),
            expanded: false,
            children: vec![],
        }
    }

    pub fn flatten(&self, depth: u16) -> Vec<FlatNode> {
        let mut result = vec![FlatNode {
            label: self.label.clone(),
            depth,
            expanded: self.expanded,
            has_children: !self.children.is_empty(),
        }];
        if self.expanded {
            for child in &self.children {
                result.extend(child.flatten(depth + 1));
            }
        }
        result
    }

    pub fn flatten_all(roots: &[TreeNode]) -> Vec<FlatNode> {
        let mut items = Vec::new();
        for node in roots {
            items.extend(node.flatten(0));
        }
        items
    }

    pub fn toggle_at_index(nodes: &mut [TreeNode], flat_index: usize) {
        let mut counter = 0;
        Self::walk_mut(nodes, flat_index, &mut counter, |node| {
            if !node.children.is_empty() {
                node.expanded = !node.expanded;
            }
        });
    }

    pub fn collapse_at_index(nodes: &mut [TreeNode], flat_index: usize) {
        let mut counter = 0;
        Self::walk_mut(nodes, flat_index, &mut counter, |node| {
            if !node.children.is_empty() && node.expanded {
                node.expanded = false;
            }
        });
    }

    pub fn walk_mut(
        nodes: &mut [TreeNode],
        target: usize,
        counter: &mut usize,
        action: impl Fn(&mut TreeNode) + Copy,
    ) -> bool {
        for node in nodes.iter_mut() {
            if *counter == target {
                action(node);
                return true;
            }
            *counter += 1;
            if node.expanded {
                if Self::walk_mut(&mut node.children, target, counter, action) {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tree() -> Vec<TreeNode> {
        vec![
            TreeNode::connection(
                "conn1",
                vec![
                    TreeNode::folder(
                        "Tables",
                        vec![TreeNode::leaf("users"), TreeNode::leaf("orders")],
                    ),
                    TreeNode::folder("Views", vec![TreeNode::leaf("summary")]),
                ],
            ),
            TreeNode::connection("conn2", vec![]),
        ]
    }

    #[test]
    fn flatten_all_collapsed() {
        let tree = sample_tree();
        let flat = TreeNode::flatten_all(&tree);
        // connections start collapsed, so only top-level nodes visible
        let labels: Vec<&str> = flat.iter().map(|n| n.label.as_str()).collect();
        assert_eq!(labels, vec!["conn1", "conn2"]);
    }

    #[test]
    fn flatten_after_expanding_connection() {
        let mut tree = sample_tree();
        // Expand conn1
        TreeNode::toggle_at_index(&mut tree, 0);
        let flat = TreeNode::flatten_all(&tree);
        let labels: Vec<&str> = flat.iter().map(|n| n.label.as_str()).collect();
        assert_eq!(labels, vec!["conn1", "Tables", "Views", "conn2"]);
        let depths: Vec<u16> = flat.iter().map(|n| n.depth).collect();
        assert_eq!(depths, vec![0, 1, 1, 0]);
    }

    #[test]
    fn toggle_expands_collapsed_folder() {
        let mut tree = sample_tree();
        // Expand conn1 first, then expand Tables (index 1)
        TreeNode::toggle_at_index(&mut tree, 0);
        TreeNode::toggle_at_index(&mut tree, 1);
        let flat = TreeNode::flatten_all(&tree);
        let labels: Vec<&str> = flat.iter().map(|n| n.label.as_str()).collect();
        assert_eq!(
            labels,
            vec!["conn1", "Tables", "users", "orders", "Views", "conn2"]
        );
    }

    #[test]
    fn toggle_collapses_expanded_node() {
        let mut tree = sample_tree();
        // Expand conn1 then collapse it
        TreeNode::toggle_at_index(&mut tree, 0);
        TreeNode::toggle_at_index(&mut tree, 0);
        let flat = TreeNode::flatten_all(&tree);
        let labels: Vec<&str> = flat.iter().map(|n| n.label.as_str()).collect();
        assert_eq!(labels, vec!["conn1", "conn2"]);
    }

    #[test]
    fn collapse_on_already_collapsed_is_noop() {
        let mut tree = sample_tree();
        let before = TreeNode::flatten_all(&tree).len();
        // conn1 at index 0 is already collapsed
        TreeNode::collapse_at_index(&mut tree, 0);
        let after = TreeNode::flatten_all(&tree).len();
        assert_eq!(before, after);
    }

    #[test]
    fn collapse_shrinks_expanded_node() {
        let mut tree = sample_tree();
        // Expand conn1, then expand Tables
        TreeNode::toggle_at_index(&mut tree, 0);
        TreeNode::toggle_at_index(&mut tree, 1);
        assert_eq!(TreeNode::flatten_all(&tree).len(), 6);
        // Collapse Tables
        TreeNode::collapse_at_index(&mut tree, 1);
        assert_eq!(TreeNode::flatten_all(&tree).len(), 4);
    }

    #[test]
    fn toggle_leaf_is_noop() {
        let mut tree = sample_tree();
        // Expand conn1, then expand Tables so leaves are visible
        TreeNode::toggle_at_index(&mut tree, 0);
        TreeNode::toggle_at_index(&mut tree, 1);
        let before = TreeNode::flatten_all(&tree).len();
        // "users" is now at index 2, a leaf
        TreeNode::toggle_at_index(&mut tree, 2);
        let after = TreeNode::flatten_all(&tree).len();
        assert_eq!(before, after);
    }

    #[test]
    fn flat_node_has_children_flag() {
        let mut tree = sample_tree();
        // Expand conn1 to see its children
        TreeNode::toggle_at_index(&mut tree, 0);
        let flat = TreeNode::flatten_all(&tree);
        // conn1 has children, Tables has children, Views has children, conn2 has no children
        let flags: Vec<bool> = flat.iter().map(|n| n.has_children).collect();
        assert_eq!(flags, vec![true, true, true, false]);
    }
}
