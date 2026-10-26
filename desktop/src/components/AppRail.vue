<template>
  <nav class="flex w-22 shrink-0 flex-col items-center gap-3 py-5">
    <div class="grid size-10 place-items-center rounded-control-lg bg-ink" role="img" aria-label="Vokia">
      <svg class="size-6" viewBox="0 0 64 64" aria-hidden="true">
        <path d="M6 16 H20 L32 44 L44 16 H58 L38 60 H26 Z" class="fill-bg" />
        <path
          d="M32 40 C 24 34, 23 25, 28 18 C 31 14, 31 9, 29 3 C 38 7, 43 15, 41 24 C 40 29, 37 33, 32 40 Z"
          class="fill-bg"
        />
        <path d="M32 44 L36 34 C 34 37, 33 39, 32 40 C 31 39, 30 37, 28 34 Z" class="fill-ink" />
      </svg>
    </div>
    <div class="flex flex-col gap-0.5 rounded-card border border-line bg-surface p-1.25 shadow-float">
      <a
        class="grid size-9.5 place-items-center rounded-control bg-accent-soft text-accent"
        aria-current="page"
        aria-label="Conversas"
        title="Conversas"
      >
        <svg
          class="size-4"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.4"
          aria-hidden="true"
        >
          <path d="M4 5h16v11H8l-4 4z" stroke-linejoin="round" />
        </svg>
      </a>
    </div>
  </nav>
</template>
