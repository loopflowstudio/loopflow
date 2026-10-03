// Homepage product window: steps through the captures, or lets the reader choose one.
(() => {
  const stage = document.querySelector('[data-stage]');
  if (!stage) return;
  const tabs = [...document.querySelectorAll('.home-stage-tabs button')];
  const shots = [...stage.querySelectorAll('.home-shot')];
  const caption = document.querySelector('.home-stage-caption');
  const order = tabs.map((tab) => tab.dataset.frame);
  const still = matchMedia('(prefers-reduced-motion: reduce)').matches;
  let timers = [];
  let auto = !still && order.length > 1;

  const later = (fn, ms) => timers.push(setTimeout(fn, ms));
  const clear = () => {
    timers.forEach(clearTimeout);
    timers = [];
    stage.classList.remove('is-aiming', 'is-pressing');
  };
  const show = (frame) => {
    const index = order.indexOf(frame);
    stage.dataset.state = index === 0 ? 'first' : 'later';
    shots.forEach((shot) => {
      const at = order.indexOf(shot.dataset.frame);
      shot.classList.toggle('is-current', at === index);
      shot.classList.toggle('is-past', at < index);
    });
    for (const tab of tabs) {
      tab.setAttribute('aria-pressed', String(tab.dataset.frame === frame));
      if (tab.dataset.frame === frame) caption.textContent = tab.dataset.caption;
    }
  };
  const current = () => stage.querySelector('.home-shot.is-current').dataset.frame;
  const loop = () => {
    clear();
    show(order[0]);
    later(() => stage.classList.add('is-aiming'), 1800);
    later(() => stage.classList.add('is-pressing'), 2900);
    order.slice(1).forEach((frame, step) => {
      later(() => {
        stage.classList.remove('is-aiming', 'is-pressing');
        show(frame);
      }, 3400 + step * 4000);
    });
    later(loop, 3400 + (order.length - 1) * 4000 + 600);
  };
  const choose = (frame) => {
    auto = false;
    clear();
    show(frame);
  };

  for (const tab of tabs) tab.addEventListener('click', () => choose(tab.dataset.frame));
  stage.addEventListener('click', () => choose(order[(order.indexOf(current()) + 1) % order.length]));
  new IntersectionObserver(([entry]) => {
    if (!auto) return;
    if (entry.isIntersecting) loop();
    else clear();
  }, { threshold: 0.4 }).observe(stage);
})();
