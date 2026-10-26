<template>
  <nav class="flex w-[88px] shrink-0 flex-col items-center gap-6 border-r border-line bg-surface py-5">
    <div class="grid h-10 w-10 place-items-center rounded-xl bg-ink" role="img" aria-label="Vokia">
      <svg class="h-6 w-6" viewBox="0 0 64 64" aria-hidden="true">
        <path d="M6 16 H20 L32 44 L44 16 H58 L38 60 H26 Z" class="fill-canvas" />
        <path
          d="M32 40 C 24 34, 23 25, 28 18 C 31 14, 31 9, 29 3 C 38 7, 43 15, 41 24 C 40 29, 37 33, 32 40 Z"
          class="fill-canvas"
        />
        <path d="M32 44 L36 34 C 34 37, 33 39, 32 40 C 31 39, 30 37, 28 34 Z" class="fill-ink" />
      </svg>
    </div>
    <a
      class="flex w-16 flex-col items-center gap-1 rounded-xl bg-accent-soft py-2 text-xs font-medium text-accent"
      aria-current="page"
    >
      <svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
        <path d="M4 5h16v11H8l-4 4z" stroke-linejoin="round" />
      </svg>
      Conversas
    </a>
  </nav>
</template>
