Phoenix.set({
  daemon: false,
  openAtLogin: true,
});

const PHOENIX_KEYS = ["cmd", "option"];
const CODING_SPLIT_INDEX = 0.58;

/** @type {(key: string, (opts: { screen: any, window: any }) => )} */
function onKey(key, callback) {
  Key.on(key, PHOENIX_KEYS, function () {
    const screen = Screen.main().flippedVisibleFrame();
    const window = Window.focused();

    if (window) {
      callback({ screen, window });
    }
  });
}

function moveToScreen(window, screenNumber) {
  if (!window) return;
  const allScreens = Screen.all();

  if (allScreens.length < 2) {
    return;
  }

  const screen = allScreens[screenNumber - 1];

  var frame = window.frame();
  var oldScreenRect = window.screen().visibleFrameInRectangle();
  var newScreenRect = screen.flippedVisibleFrame();
  var xRatio = newScreenRect.width / oldScreenRect.width;
  var yRatio = newScreenRect.height / oldScreenRect.height;

  var mid_pos_x = frame.x + Math.round(0.5 * frame.width);
  var mid_pos_y = frame.y + Math.round(0.5 * frame.height);

  window.setFrame({
    x:
      (mid_pos_x - oldScreenRect.x) * xRatio +
      newScreenRect.x -
      0.5 * frame.width,
    y:
      (mid_pos_y - oldScreenRect.y) * yRatio +
      newScreenRect.y -
      0.5 * frame.height,
    width: frame.width,
    height: frame.height,
  });
}

onKey("up", ({ window }) => window.maximize());

onKey("down", ({ screen, window }) => {
  window.setSize({
    width: 1920,
    height: 1080,
  });

  window.setTopLeft({
    x: screen.x + screen.width / 2 - window.frame().width / 2,
    y: screen.y + screen.height / 2 - window.frame().height / 2,
  });
});

onKey("left", ({ screen, window }) => {
  window.setTopLeft({
    x: screen.x,
    y: screen.y,
  });

  window.setSize({
    width: screen.width / 2,
    height: screen.height,
  });
});

onKey("right", ({ screen, window }) => {
  window.setTopLeft({
    x: screen.x + screen.width / 2,
    y: screen.y,
  });

  window.setSize({
    width: screen.width / 2,
    height: screen.height,
  });
});

onKey(",", ({ screen, window }) => {
  window.setTopLeft({
    x: screen.x,
    y: screen.y,
  });

  window.setSize({
    width: screen.width * CODING_SPLIT_INDEX,
    height: screen.height,
  });
});

onKey(".", ({ screen, window }) => {
  window.setTopLeft({
    x: screen.x + screen.width * CODING_SPLIT_INDEX,
    y: screen.y,
  });

  window.setSize({
    width: screen.width * (1 - CODING_SPLIT_INDEX),
    height: screen.height,
  });
});

onKey("R", ({ window }) => {
  moveToScreen(window, 2);
  window.focus();
});

onKey("T", ({ window }) => {
  moveToScreen(window, 1);
  window.focus();
});
