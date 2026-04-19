vim.api.nvim_create_user_command("LazyDB", function()
  require("lazydb").toggle()
end, {})

vim.api.nvim_create_user_command("LazyDBOpen", function()
  require("lazydb").open()
end, {})

vim.api.nvim_create_user_command("LazyDBClose", function()
  require("lazydb").close()
end, {})
