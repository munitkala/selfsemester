<script lang="ts">
  import { onMount } from "svelte";
  import { api, live, type CourseCard, type InsightsData } from "#lib/api.ts";
  import { prefs } from "#lib/prefs.svelte.ts";
  import Icon from "#lib/Icon.svelte";
  import Heatmap from "#lib/charts/Heatmap.svelte";
  import Bars from "#lib/charts/Bars.svelte";
  import MarkTrend from "#lib/charts/MarkTrend.svelte";
  import Ring from "#lib/charts/Ring.svelte";
  import {
    averageByKind,
    byDay,
    byHour,
    byWeekday,
    formatMinutes,
    heatmap,
    lastDays,
    markTrend,
    minutesByCourse,
    observations,
    onTime,
    streaks,
    weekCompare,
    workload,
  } from "#lib/insights.ts";

  let data = $state<InsightsData | null>(null);
  let courses = $state<CourseCard[]>([]);
  let error = $state<string | null>(null);
  let range = $state<14 | 30 | 90>(30);

  onMount(() =>
    live(async () => {
      try {
        [data, courses] = await Promise.all([api.insights(), api.courses()]);
        error = null;
      } catch (e) {
        error = String(e);
      }
    }),
  );

  const days = $derived(data ? byDay(data) : new Map());
  const streak = $derived(streaks(days));
  const grid = $derived(heatmap(days, prefs.heatmapWeeks, prefs.weekStart));
  const recent = $derived(lastDays(days, range));
  const rangeTotal = $derived(recent.reduce((s, d) => s + d.minutes, 0));
  const studiedDays = $derived(recent.filter((d) => d.minutes >= 1).length);
  const week = $derived(weekCompare(days, prefs.weekStart));
  const todayMinutes = $derived(recent[recent.length - 1]?.minutes ?? 0);
  const trend = $derived(data ? markTrend(data, courses) : []);
  const kinds = $derived(data ? averageByKind(data) : []);
  const timeliness = $derived(data ? onTime(data) : null);
  const perCourse = $derived(data ? minutesByCourse(data, courses, range) : []);
  const hours = $derived(data ? byHour(data) : []);
  const weekdays = $derived(data ? byWeekday(data) : []);
  const ahead = $derived(data ? workload(data, 6, prefs.weekStart) : []);
  const notes = $derived(data ? observations(data, days, prefs.weekStart) : []);
  const average = $derived(trend.length ? trend.reduce((s, p) => s + p.percent, 0) / trend.length : null);
  const empty = $derived(!!data && !data.study.length && !data.marks.length && !data.events.length);

  const dayLabel = (day: string) => new Date(day + "T12:00:00").toLocaleDateString(undefined, { month: "short", day: "numeric" });
  const shortDay = (i: number) => ["S", "M", "T", "W", "T", "F", "S"][i];
  const hourLabel = (h: number) => new Date(2000, 0, 1, h).toLocaleTimeString(undefined, { hour: "numeric" });
  const weekdayOrder = $derived(Array.from({ length: 7 }, (_, i) => (i + prefs.weekStart) % 7));
</script>

<div class="page-enter">
  <div class="spread">
    <div>
      <h1>Insights</h1>
      <p class="lead">How you've been studying, and how it's going.</p>
    </div>
    <div class="segmented" role="group" aria-label="Range">
      {#each [14, 30, 90] as r}
        <button aria-pressed={range === r} onclick={() => (range = r as 14 | 30 | 90)}>{r} days</button>
      {/each}
    </div>
  </div>

  {#if error}<p class="note bad">{error}</p>{/if}
  {#if !prefs.trackStudy}
    <p class="note info small">Study time tracking is off, so only marks and handed-in work show here. Turn it on in <a href="/settings#study">Settings</a>.</p>
  {/if}

  {#if !data}
    <p class="muted">Loading…</p>
  {:else}
    {#if empty}
      <section class="card empty">
        <Icon name="sparkle" size={28} />
        <div>
          <h2>Nothing to show yet</h2>
          <p class="muted">Open a lesson or some homework and your study time starts counting. Marks and streaks show up here as you go.</p>
        </div>
      </section>
    {/if}

    <section class="tiles">
      <div class="tile">
        <span class="icon"><Icon name="flame" /></span>
        <div>
          <strong>{streak.current} {streak.current === 1 ? "day" : "days"}</strong>
          <span class="small muted">Streak · best {streak.best}</span>
        </div>
      </div>
      <div class="tile">
        <span class="icon"><Icon name="clock" /></span>
        <div>
          <strong>{formatMinutes(rangeTotal)}</strong>
          <span class="small muted">Last {range} days · {studiedDays} days studied</span>
        </div>
      </div>
      <div class="tile">
        <span class="icon"><Icon name="target" /></span>
        <div>
          <strong>{average == null ? "-" : `${average.toFixed(0)}%`}</strong>
          <span class="small muted">Average mark · {trend.length} marked</span>
        </div>
      </div>
      <div class="tile">
        <span class="icon"><Icon name="check" /></span>
        <div>
          <strong>{timeliness?.rate == null ? "-" : `${timeliness.rate.toFixed(0)}%`}</strong>
          <span class="small muted">On time{timeliness && timeliness.late + timeliness.missed ? ` · ${timeliness.late} late, ${timeliness.missed} missed` : ""}</span>
        </div>
      </div>
    </section>

    {#if notes.length}
      <section class="card notes">
        <h2><Icon name="sparkle" size={16} /> What stands out</h2>
        <ul>
          {#each notes as n}<li>{n}</li>{/each}
        </ul>
      </section>
    {/if}

    <section class="card">
      <div class="spread">
        <h2>Activity</h2>
        <span class="small muted">Last {prefs.heatmapWeeks} weeks</span>
      </div>
      <Heatmap {grid} weekStart={prefs.weekStart} />
    </section>

    <div class="two">
      <section class="card">
        <div class="spread">
          <h2>Study time</h2>
          {#if prefs.dailyGoal}<span class="small muted">Dashed line: {prefs.dailyGoal} min goal</span>{/if}
        </div>
        <Bars
          data={recent.map((d, i) => ({ label: dayLabel(d.day), value: d.minutes, title: `${dayLabel(d.day)}: ${formatMinutes(d.minutes)}`, highlight: i === recent.length - 1 }))}
          goal={prefs.dailyGoal}
          format={formatMinutes}
          every={range === 14 ? 2 : range === 30 ? 5 : 15}
          label="Minutes studied per day"
        />
      </section>
      <section class="card week">
        <h2>Today and this week</h2>
        <div class="row">
          <Ring value={prefs.dailyGoal ? todayMinutes / prefs.dailyGoal : 0} size={96} stroke={10} label="Today's study time against your daily goal">
            <div><strong>{Math.round(todayMinutes)}</strong><br /><span class="small muted">{prefs.dailyGoal ? `of ${prefs.dailyGoal} min` : "min"}</span></div>
          </Ring>
          <div class="small">
            <p><strong>{formatMinutes(week.thisWeek)}</strong> this week</p>
            <p class="muted">{formatMinutes(week.lastWeek)} by this point last week</p>
          </div>
        </div>
      </section>
    </div>

    <section class="card offscreen-lazy">
      <h2>Marks over time</h2>
      {#if trend.length}
        <MarkTrend points={trend} />
        <p class="small muted">Each dot is a marked piece of work, in its course's colour. The line is the average of the last five.</p>
      {:else}
        <p class="muted small">Marks show here once work comes back.</p>
      {/if}
    </section>

    <div class="two offscreen-lazy">
      <section class="card">
        <h2>By type of work</h2>
        {#if kinds.length}
          <ul class="meters">
            {#each kinds as k}
              <li>
                <span>{k.label} <span class="muted small">({k.count})</span></span>
                <span class="meter"><span style="width: {k.average}%"></span></span>
                <strong>{k.average.toFixed(0)}%</strong>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="muted small">No marks yet.</p>
        {/if}
      </section>
      <section class="card">
        <h2>Time per course</h2>
        {#if perCourse.length}
          <ul class="meters">
            {#each perCourse as c}
              {@const top = perCourse[0].minutes || 1}
              <li>
                <a href="/course/{encodeURIComponent(c.id)}" class="name">{c.title}</a>
                <span class="meter"><span style="width: {(100 * c.minutes) / top}%; --tone: {c.color}"></span></span>
                <strong>{formatMinutes(c.minutes)}</strong>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="muted small">No study time in the last {range} days.</p>
        {/if}
      </section>
    </div>

    <div class="two offscreen-lazy">
      <section class="card">
        <h2>When you study</h2>
        {#if hours.some((h) => h > 0)}
          <Bars data={hours.map((m, h) => ({ label: hourLabel(h), value: m, title: `${hourLabel(h)}: ${formatMinutes(m)}` }))} format={formatMinutes} every={6} height={120} label="Minutes studied by hour of day" />
          <Bars data={weekdayOrder.map((d) => ({ label: shortDay(d), value: weekdays[d], title: formatMinutes(weekdays[d]) }))} format={formatMinutes} height={100} label="Minutes studied by weekday" />
        {:else}
          <p class="muted small">Shows once some study time is counted.</p>
        {/if}
      </section>
      <section class="card">
        <h2>Coming up</h2>
        <Bars
          data={ahead.map((w, i) => ({
            label: i === 0 ? "This wk" : w.start.toLocaleDateString(undefined, { month: "short", day: "numeric" }),
            value: w.count,
            title: `${w.count} due, ${(w.weight * 100).toFixed(0)}% of grades`,
            highlight: i === 0,
          }))}
          height={120}
          label="Graded work due per week"
        />
        <p class="small muted">Graded work due in each of the next six weeks.</p>
      </section>
    </div>
  {/if}
</div>

<style>
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(10rem, 1fr));
    gap: var(--space);
    margin-bottom: var(--space);
  }
  .tile {
    display: flex;
    gap: 0.8rem;
    align-items: center;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 0.9rem 1rem;
    box-shadow: var(--shadow);
  }
  .tile strong {
    display: block;
    font-size: 1.35rem;
    line-height: 1.2;
  }
  .icon {
    display: grid;
    place-items: center;
    width: 2.4rem;
    height: 2.4rem;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
    flex: none;
  }
  .two {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(19rem, 1fr));
    gap: var(--space);
  }
  .two > .card {
    margin-bottom: var(--space);
  }
  .notes h2 {
    display: flex;
    gap: 0.4rem;
    align-items: center;
  }
  .notes ul {
    margin: 0;
    padding-left: 1.2rem;
  }
  .week p {
    margin: 0.2rem 0;
  }
  .empty {
    display: flex;
    gap: 1rem;
    align-items: flex-start;
    color: var(--accent);
  }
  .empty h2,
  .empty p {
    color: var(--text);
    margin: 0 0 0.3rem;
  }
  .meters {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: 0.55rem;
  }
  .meters li {
    display: grid;
    grid-template-columns: minmax(6rem, 1fr) 2fr auto;
    gap: 0.6rem;
    align-items: center;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meter {
    height: 8px;
    border-radius: 4px;
    background: var(--line);
    overflow: hidden;
  }
  .meter span {
    display: block;
    height: 100%;
    border-radius: 4px;
    background: color-mix(in oklch, var(--tone, var(--accent)) var(--course-lift), white);
  }
</style>
