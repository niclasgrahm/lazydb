vim.api.nvim_create_user_command("LazyDB", function()
  require("lazydb").toggle()
end, {})

vim.api.nvim_create_user_command("LazyDBOpen", function()
  require("lazydb").open()
end, {})

vim.api.nvim_create_user_command("LazyDBHide", function()
  require("lazydb").hide()
end, {})

vim.api.nvim_create_user_command("LazyDBKill", function()
  require("lazydb").kill()
end, {})
