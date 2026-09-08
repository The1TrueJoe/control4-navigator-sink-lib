--[[
  Control4 Navigator Sink — DriverWorks driver (controller side)
  ============================================================
  Companion to control4-navigator-sink-lib. Presents the on-screen navigator
  proxies (controller 5001 + uidevice 5002), an HDMI output (4072) and the
  ONSCREEN_SELECTION room endpoint (7500), and relays every command Director
  sends it to the Rust sink as **NDJSON** (one JSON object per line) over a
  persistent TCP socket.

  Frame shapes (match src/protocol.rs):
    {"type":"hello","message":"..."}
    {"type":"bind","binding":7500,"class":"ONSCREEN_SELECTION","bound":true}
    {"type":"command","binding":5001,"room":31,"command":"UP","params":{...}}
]]--

TargetIP   = "192.168.1.146"
TargetPort = 9010

local gClient        -- persistent TCP client
local gConnected = false
local gQueue = {}     -- outbound line buffer (flushed on connect)

local function log(...) print("[nav-sink] " .. table.concat({...}, " ")) end

-- ---- minimal JSON encoding (escape so every frame stays on one line) --------
local function jstr(s)
  s = tostring(s)
  s = s:gsub('[%z\1-\31\\"]', function(c)
    local map = { ['"']='\\"', ['\\']='\\\\', ['\n']='\\n', ['\r']='\\r', ['\t']='\\t' }
    return map[c] or string.format('\\u%04x', string.byte(c))
  end)
  return '"' .. s .. '"'
end

local function jparams(t)
  if type(t) ~= "table" then return "{}" end
  local parts = {}
  for k, v in pairs(t) do parts[#parts+1] = jstr(k) .. ":" .. jstr(v) end
  return "{" .. table.concat(parts, ",") .. "}"
end

-- ---- persistent transport ---------------------------------------------------
local function flush()
  if not (gConnected and gClient) then return end
  for _, line in ipairs(gQueue) do pcall(function() gClient:Write(line .. "\n") end) end
  gQueue = {}
end

local function connect()
  gClient = C4:CreateTCPClient()
    :OnConnect(function(_)
      gConnected = true
      C4:UpdateProperty("Link Status", "connected " .. TargetIP .. ":" .. TargetPort)
      flush()
    end)
    :OnDisconnect(function(_, _)
      gConnected = false
      C4:UpdateProperty("Link Status", "disconnected")
      C4:SetTimer(3000, connect)
    end)
    :OnError(function(_, err)
      gConnected = false
      C4:UpdateProperty("Link Status", "error: " .. tostring(err))
      C4:SetTimer(3000, connect)
    end)
    :Connect(TargetIP, TargetPort)
end

local function send(line)
  gQueue[#gQueue + 1] = line
  if gConnected then flush() else if not gClient then connect() end end
end

local function emit_command(binding, room, command, tParams)
  local r = room and tostring(room) or "null"
  send(string.format('{"type":"command","binding":%s,"room":%s,"command":%s,"params":%s}',
    tostring(binding), r, jstr(command), jparams(tParams)))
end

-- ---- DriverWorks entry points ----------------------------------------------
function ReceivedFromProxy(idBinding, strCommand, tParams)
  tParams = tParams or {}
  local room = tParams.ROOM_ID or tParams.ROOMID
  C4:UpdateProperty("Last Command", tostring(strCommand))
  emit_command(idBinding, room, strCommand, tParams)
  if strCommand == "GET_CONTROLLER_DISABLED" then
    C4:SendToProxy(idBinding, "CONTROLLER_DISABLED", { STATE = "false" })
  end
end

function OnBindingChanged(idBinding, class, bIsBound)
  send(string.format('{"type":"bind","binding":%s,"class":%s,"bound":%s}',
    tostring(idBinding), jstr(class), bIsBound and "true" or "false"))
end

function ExecuteCommand(strCommand, tParams)
  if strCommand == "TEST_LINK" then send('{"type":"hello","message":"manual test line"}') end
end

function OnPropertyChanged(strProperty)
  local v = Properties[strProperty]
  if strProperty == "Target IP Address" then TargetIP = v
  elseif strProperty == "Target Port" then TargetPort = tonumber(v) or TargetPort end
end

function OnDriverInit() end

function OnDriverLateInit()
  TargetIP   = Properties["Target IP Address"] or TargetIP
  TargetPort = tonumber(Properties["Target Port"]) or TargetPort
  connect()
  send('{"type":"hello","message":"Control4 Navigator Sink online"}')
end

function OnDriverDestroyed()
  if gClient then pcall(function() gClient:Close() end) end
end
