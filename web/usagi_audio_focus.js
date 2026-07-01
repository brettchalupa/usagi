// Baked into usagi.js via --pre-js so every shell (default or custom) gets it.
// Suspends Web Audio while the tab is hidden so streaming music doesn't
// starve and stutter: the render loop stops on hidden tabs, and raylib's
// focus API doesn't track tab visibility. miniaudio makes a few contexts;
// capture them all and act on whichever is live.
window.__usagiAudioCtxs = [];
(function () {
  var AC = window.AudioContext || window.webkitAudioContext;
  if (!AC) return;
  var Patched = function () {
    var ctx = Reflect.construct(AC, arguments);
    window.__usagiAudioCtxs.push(ctx);
    return ctx;
  };
  Patched.prototype = AC.prototype;
  window.AudioContext = Patched;
  if (window.webkitAudioContext) window.webkitAudioContext = Patched;
})();
document.addEventListener("visibilitychange", function () {
  (window.__usagiAudioCtxs || []).forEach(function (ctx) {
    if (ctx.state === "closed") return;
    (document.hidden ? ctx.suspend() : ctx.resume()).catch(function () {});
  });
});
