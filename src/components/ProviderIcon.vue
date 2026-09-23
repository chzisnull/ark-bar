<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    name: string;
    class?: any;
  }>(),
  {
    class: 'w-5 h-5',
  }
);
</script>

<template>
  <span :class="['inline-flex items-center justify-center shrink-0', props.class]">
    <!-- Antigravity (Arch / Chevron A logo from Codenotch screenshots) -->
    <svg
      v-if="props.name === 'antigravity'"
      viewBox="0 0 24 24"
      fill="currentColor"
      class="w-full h-full"
    >
      <path
        d="M12 2.8C9.2 2.8 5.6 7.4 3.8 15.2c-.6 2.6-.8 4.2-.8 4.8 0 .8.5 1.2 1.3 1.2 1.2 0 2.2-.9 3-2.6 1.1-2.4 2.6-6.1 4.7-11 2.1 4.9 3.6 8.6 4.7 11 .8 1.7 1.8 2.6 3 2.6.8 0 1.3-.4 1.3-1.2 0-.6-.2-2.2-.8-4.8C18.4 7.4 14.8 2.8 12 2.8z"
      />
    </svg>

    <!-- Grok (Compass orbit slash logo from Codenotch screenshots) -->
    <svg
      v-else-if="props.name === 'grok'"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2.2"
      stroke-linecap="round"
      stroke-linejoin="round"
      class="w-full h-full"
    >
      <circle cx="12" cy="12" r="8.2" />
      <path d="M5.5 18.5L18.5 5.5" />
      <path d="M12 5.5h6.5v6.5" stroke-width="0" fill="currentColor" />
    </svg>

    <!-- Volcengine Ark (Volcano Flame logo) -->
    <svg
      v-else-if="props.name === 'volcengine'"
      viewBox="0 0 24 24"
      fill="currentColor"
      class="w-full h-full"
    >
      <path
        d="M12 2C8.8 6.8 5 10.4 5 15.2a7 7 0 0 0 14 0c0-4.8-3.8-8.4-7-13.2zm0 16.8a4.8 4.8 0 0 1-4.8-4.8c0-2.4 2-5 4.8-8.2 2.8 3.2 4.8 5.8 4.8 8.2a4.8 4.8 0 0 1-4.8 4.8z"
      />
      <path d="M12 13.5c-1.2 1.5-1.8 2.7-1.8 3.8a1.8 1.8 0 0 0 3.6 0c0-1.1-.6-2.3-1.8-3.8z" />
    </svg>

    <!-- OpenAI Codex (AI Rosette / Terminal) -->
    <svg
      v-else-if="props.name === 'codex'"
      viewBox="0 0 24 24"
      fill="currentColor"
      class="w-full h-full"
    >
      <path
        d="M21.2 10.4a5.3 5.3 0 0 0-.4-4.2 5.4 5.4 0 0 0-5.1-2.7 5.3 5.3 0 0 0-4-1.8 5.4 5.4 0 0 0-5 3.5 5.3 5.3 0 0 0-3.6 2.6 5.4 5.4 0 0 0 .5 5.7 5.3 5.3 0 0 0 .4 4.2 5.4 5.4 0 0 0 5.1 2.7 5.3 5.3 0 0 0 4 1.8 5.4 5.4 0 0 0 5-3.5 5.3 5.3 0 0 0 3.6-2.6 5.4 5.4 0 0 0-.5-5.7zm-7.9 9.3a3.6 3.6 0 0 1-2.5-.9l3.1-1.8a1.8 1.8 0 0 0 .9-.9v-3.7l2.1 1.2v3.6a3.6 3.6 0 0 1-3.6 2.5zm-7.6-3.8a3.6 3.6 0 0 1-.5-2.6l3.1 1.8a1.8 1.8 0 0 0 1.3 0l3.2-1.8v2.4l-3.2 1.8a3.6 3.6 0 0 1-3.9-1.6zm-1.4-8.2a3.6 3.6 0 0 1 2-1.7v3.6a1.8 1.8 0 0 0 .4 1.3l3.2 1.8-2.1 1.2-3.1-1.8a3.6 3.6 0 0 1-.4-4.4zm11.8 3.5l-3.2-1.8 2.1-1.2 3.1 1.8a3.6 3.6 0 0 1-.5 6.1v-3.6a1.8 1.8 0 0 0-.4-1.3zm2.5-3.2l-3.1 1.8a1.8 1.8 0 0 0-1.3 0L10.6 8V5.6l3.2-1.8a3.6 3.6 0 0 1 4.5 3.2v.2zm-7.6-4.3a3.6 3.6 0 0 1 2.5.9L9.8 5.4a1.8 1.8 0 0 0-.9.9V10L6.8 8.8V5.2a3.6 3.6 0 0 1 3.6-2.5z"
      />
    </svg>

    <!-- TeamoRouter (Satellite Dish & Router waves) -->
    <svg
      v-else-if="props.name === 'teamo'"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      class="w-full h-full"
    >
      <path d="M4 10a12 12 0 0 1 16 0" />
      <path d="M7 13a8 8 0 0 1 10 0" />
      <path d="M10 16a4 4 0 0 1 4 0" />
      <circle cx="12" cy="19" r="1.5" fill="currentColor" />
    </svg>

    <!-- Claude (Anthropic Starburst) -->
    <svg
      v-else-if="props.name === 'claude'"
      viewBox="0 0 24 24"
      fill="currentColor"
      class="w-full h-full"
    >
      <path
        d="M12 2l1.6 5.8L19.4 6 15 10.4l5.8 1.6-5.8 1.6 4.4 4.4-5.8-1.6L12 22l-1.6-5.8L4.6 18 9 13.6 3.2 12 9 10.4 4.6 6l5.8 1.8L12 2z"
      />
    </svg>

    <!-- Cursor (Isometric Arrow Cube) -->
    <svg
      v-else-if="props.name === 'cursor'"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      class="w-full h-full"
    >
      <path d="M12 2L3 7v10l9 5 9-5V7l-9-5z" />
      <path d="M12 12L3 7" />
      <path d="M12 12v10" />
      <path d="M12 12l9-5" />
      <path d="M12 7l4 2.5-4 2.5-4-2.5z" fill="currentColor" opacity="0.3" />
    </svg>

    <!-- DeepSeek (Whale) -->
    <svg
      v-else-if="props.name === 'deepseek'"
      viewBox="0 0 24 24"
      fill="currentColor"
      class="w-full h-full"
    >
      <path
        d="M20 13c-1.5-3.5-4.5-5.5-8.5-5.5C6 7.5 2 11.5 2 16.5c0 1.5.5 2.5 1.5 2.5 3 0 5-2 8-2 2.5 0 4.5 1.5 6.5 1.5 1.5 0 2.5-1 2.5-2.5 0-1-.2-2-.5-3zm-14 2c-.6 0-1-.4-1-1s.4-1 1-1 1 .4 1 1-.4 1-1 1zm14.5-7.5c-.8.8-2 .5-2.8 0 .8-.8 2-.5 2.8 0z"
      />
    </svg>

    <!-- Ollama (Llama) -->
    <svg
      v-else-if="props.name === 'ollama'"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      class="w-full h-full"
    >
      <path d="M18 10h-2V6a2 2 0 0 0-4 0v4H8a4 4 0 0 0-4 4v5a1 1 0 0 0 1 1h2v-3h10v3h2a1 1 0 0 0 1-1v-5a4 4 0 0 0-4-4z" />
      <circle cx="9" cy="14" r="1" fill="currentColor" />
      <circle cx="15" cy="14" r="1" fill="currentColor" />
    </svg>

    <!-- LM Studio (Stacked Layers) -->
    <svg
      v-else-if="props.name === 'lmstudio'"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      class="w-full h-full"
    >
      <path d="M4 6h16M4 12h16M4 18h16" />
      <rect x="7" y="4" width="4" height="4" rx="1" fill="currentColor" />
      <rect x="13" y="10" width="4" height="4" rx="1" fill="currentColor" />
      <rect x="9" y="16" width="4" height="4" rx="1" fill="currentColor" />
    </svg>

    <!-- Fallback icon -->
    <span v-else class="text-sm select-none">⚡</span>
  </span>
</template>
