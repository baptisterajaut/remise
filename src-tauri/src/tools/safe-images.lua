-- Remise runs Pandoc with --sandbox, which reads nothing but the input file. This filter
-- (filters are not sandboxed) lets the document's images back in, and only images:
--   * those the reader already holds (inside a .docx or .epub);
--   * relative paths that stay inside the input's folder;
--   * http(s) URLs.
-- Whatever it fetches must have an image/* type (by extension on disk, by Content-Type on
-- the web). Anything else (absolute path, .., file://, a key file, a web page, a failed
-- download) becomes its alt text, because PDF output fetches images outside the sandbox.
-- CBA: a symlink inside the folder is followed; Pandoc's Lua has no lstat.
local dir = pandoc.path.directory(PANDOC_STATE.input_files[1])

local function where(src)
  if src:find("^https?://") then
    return src
  end
  if src:find("^%a[%w+.-]*:") or src:find("^[/\\]") or src:find("%.%.") then
    return nil
  end
  return pandoc.path.join({ dir, src })
end

function Image(img)
  local src = img.src
  if pandoc.mediabag.lookup(src) then
    return nil
  end
  local from = where(src)
  if from then
    local ok, mime, contents = pcall(pandoc.mediabag.fetch, from)
    if ok and mime and mime:find("^image/") then
      pandoc.mediabag.insert(src, mime, contents)
      return nil
    end
  end
  return img.caption
end
