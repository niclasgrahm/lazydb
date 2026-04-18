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
    /// Index in the unfiltered flat list (used to map filtered selection back to tree operations).
    pub flat_index: usize,
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

    fn flatten_inner(&self, depth: u16, counter: &mut usize, out: &mut Vec<FlatNode>) {
        let idx = *counter;
        *counter += 1;
        out.push(FlatNode {
            label: self.label.clone(),
            depth,
            expanded: self.expanded,
            has_children: !self.children.is_empty(),
            flat_index: idx,
        });
        if self.expanded {
            for child in &self.children {
                child.flatten_inner(depth + 1, counter, out);
            }
        }
    }

    pub fn flatten_all(roots: &[TreeNode]) -> Vec<FlatNode> {
        let mut items = Vec::new();
        let mut counter = 0;
        for node in roots {
            node.flatten_inner(0, &mut counter, &mut items);
        }
        items
    }

    /// Flatten tree nodes, keeping only nodes (and their ancestors) whose label
    /// contains the given substring (case-insensitive). All matching subtrees
    /// are shown expanded. Each node carries its `flat_index` from the unfiltered tree.
    pub fn flatten_all_filtered(roots: &[TreeNode], filter: &str) -> Vec<FlatNode> {
        let filter_lower = filter.to_lowercase();
        let mut items = Vec::new();
        let mut counter = 0;
        for node in roots {
            Self::flatten_filtered(node, 0, &filter_lower, &mut counter, &mut items);
        }
        items
    }

    fn node_matches_recursive(node: &TreeNode, filter: &str) -> bool {
        if node.label.to_lowercase().contains(filter) {
            return true;
        }
        node.children.iter().any(|c| Self::node_matches_recursive(c, filter))
    }

    fn flatten_filtered(node: &TreeNode, depth: u16, filter: &str, counter: &mut usize, out: &mut Vec<FlatNode>) {
        let idx = *counter;
        *counter += 1;
        if !Self::node_matches_recursive(node, filter) {
            // Still need to count children for correct flat_index mapping
            Self::count_visible(node, counter);
            return;
        }
        out.push(FlatNode {
            label: node.label.clone(),
            depth,
            expanded: true,
            has_children: !node.children.is_empty(),
            flat_index: idx,
        });
        for child in &node.children {
            Self::flatten_filtered(child, depth + 1, filter, counter, out);
        }
    }

    /// Count all visible (expanded) descendants to advance the counter correctly.
    fn count_visible(node: &TreeNode, counter: &mut usize) {
        if node.expanded {
            for child in &node.children {
                *counter += 1;
                Self::count_visible(child, counter);
            }
        }
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

    #[test]
    fn filtered_flatten_matches_substring() {
        let mut tree = sample_tree();
        // Expand conn1 so children are visible in unfiltered tree
        TreeNode::toggle_at_index(&mut tree, 0);
        TreeNode::toggle_at_index(&mut tree, 1); // expand Tables

        let flat = TreeNode::flatten_all_filtered(&tree, "user");
        let labels: Vec<&str> = flat.iter().map(|n| n.label.as_str()).collect();
        // Should include conn1 (ancestor), Tables (ancestor), users (match)
        assert_eq!(labels, vec!["conn1", "Tables", "users"]);
    }

    #[test]
    fn filtered_flatten_case_insensitive() {
        let tree = sample_tree();
        let flat = TreeNode::flatten_all_filtered(&tree, "CONN1");
        let labels: Vec<&str> = flat.iter().map(|n| n.label.as_str()).collect();
        assert_eq!(labels, vec!["conn1"]);
    }

    #[test]
    fn filtered_flatten_no_match_returns_empty() {
        let tree = sample_tree();
        let flat = TreeNode::flatten_all_filtered(&tree, "nonexistent");
        assert!(flat.is_empty());
    }

    #[test]
    fn filtered_flat_index_maps_to_unfiltered() {
        let mut tree = sample_tree();
        TreeNode::toggle_at_index(&mut tree, 0); // expand conn1
        TreeNode::toggle_at_index(&mut tree, 1); // expand Tables
        // Unfiltered: conn1(0), Tables(1), users(2), orders(3), Views(4), conn2(5)
        let filtered = TreeNode::flatten_all_filtered(&tree, "orders");
        assert_eq!(filtered.len(), 3); // conn1, Tables, orders
        // "orders" should have flat_index 3
        let orders = filtered.iter().find(|n| n.label == "orders").unwrap();
        assert_eq!(orders.flat_index, 3);
    }

    #[test]
    fn empty_tree() {
        let tree: Vec<TreeNode> = vec![];
        let flat = TreeNode::flatten_all(&tree);
        assert!(flat.is_empty());
    }

    #[test]
    fn deeply_nested_tree() {
        let tree = vec![TreeNode::connection(
            "root",
            vec![TreeNode::folder(
                "level1",
                vec![TreeNode::folder(
                    "level2",
                    vec![TreeNode::folder(
                        "level3",
                        vec![TreeNode::leaf("leaf")],
                    )],
                )],
            )],
        )];
        // Expand all levels
        let mut tree = tree;
        TreeNode::toggle_at_index(&mut tree, 0); // expand root
        TreeNode::toggle_at_index(&mut tree, 1); // expand level1
        TreeNode::toggle_at_index(&mut tree, 2); // expand level2
        TreeNode::toggle_at_index(&mut tree, 3); // expand level3
        let flat = TreeNode::flatten_all(&tree);
        assert_eq!(flat.len(), 5);
        let depths: Vec<u16> = flat.iter().map(|n| n.depth).collect();
        assert_eq!(depths, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn multiple_sequential_mutations() {
        let mut tree = sample_tree();
        // Expand conn1
        TreeNode::toggle_at_index(&mut tree, 0);
        assert_eq!(TreeNode::flatten_all(&tree).len(), 4); // conn1, Tables, Views, conn2
        // Collapse conn1
        TreeNode::toggle_at_index(&mut tree, 0);
        assert_eq!(TreeNode::flatten_all(&tree).len(), 2); // conn1, conn2
        // Re-expand conn1
        TreeNode::toggle_at_index(&mut tree, 0);
        assert_eq!(TreeNode::flatten_all(&tree).len(), 4);
        // Expand Tables
        TreeNode::toggle_at_index(&mut tree, 1);
        assert_eq!(TreeNode::flatten_all(&tree).len(), 6); // conn1, Tables, users, orders, Views, conn2
    }
}
