local M = {}

M.defaults = {
  lazydb_command = "lazydb",
  floating_window_scaling_factor = 0.9,
  floating_window_winblend = 0,
  floating_window_border_chars = { "╭", "─", "╮", "│", "╯", "─", "╰", "│" },
}

M.options = vim.deepcopy(M.defaults)

function M.setup(opts)
  M.options = vim.tbl_deep_extend("force", M.defaults, opts or {})
end

return M
