export function classifyLayoutShape(width, height) {
  return classifyLayoutShapeForClient(width, height, "");
}

export function classifyLayoutShapeForClient(width, height, client) {
  const safeWidth = Math.max(1, Number(width) || 1);
  const safeHeight = Math.max(1, Number(height) || 1);
  const ratio = safeWidth / safeHeight;
  if (client === "android-webview") {
    if (ratio > 4 / 3) {
      return "phone_landscape";
    }
    if (safeWidth >= 600) {
      return "tablet_portrait";
    }
    if (ratio <= 9 / 16) {
      return "tall_phone";
    }
    return "phone_portrait";
  }
  if (safeWidth >= 1180 && ratio > 1.15) {
    return "desktop_large";
  }
  if (safeWidth >= 720 && ratio >= 0.85 && ratio <= 1.35) {
    return "foldable_unfolded";
  }
  if (safeWidth >= 880 && ratio > 1) {
    return "tablet_landscape";
  }
  if (safeWidth >= 600 && safeWidth <= 1023 && ratio <= 1) {
    return "tablet_portrait";
  }
  if (safeWidth < 880 && ratio > 4 / 3) {
    return "phone_landscape";
  }
  if (safeWidth < 720 && ratio <= 9 / 16) {
    return "tall_phone";
  }
  return "phone_portrait";
}

export function isPortraitPrimarySurfaceShape(shape) {
  return ["phone_portrait", "tall_phone", "tablet_portrait"].includes(shape);
}

export function viewportDimensionsForLayout(win = window) {
  const isAndroidWebView = new URLSearchParams(win.location.search).get("client") === "android-webview";
  const widths = [
    win.visualViewport && win.visualViewport.width,
    win.document.documentElement && win.document.documentElement.clientWidth,
    win.innerWidth,
    isAndroidWebView && win.screen && win.screen.width,
  ].map(Number).filter((value) => Number.isFinite(value) && value > 0);
  const heights = [
    win.visualViewport && win.visualViewport.height,
    win.document.documentElement && win.document.documentElement.clientHeight,
    win.innerHeight,
    isAndroidWebView && win.screen && win.screen.height,
  ].map(Number).filter((value) => Number.isFinite(value) && value > 0);
  return {
    width: widths.length > 0 ? Math.min(...widths) : 1,
    height: heights.length > 0 ? Math.max(...heights) : 1,
  };
}
