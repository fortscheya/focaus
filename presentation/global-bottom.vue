<script setup lang="ts">
import { onBeforeUnmount, onMounted } from 'vue'
let observer: IntersectionObserver | undefined
let mutations: MutationObserver | undefined
const seen = new WeakSet<Element>()
onMounted(() => {
  observer = new IntersectionObserver(entries => {
    for (const entry of entries) entry.target.classList.toggle('scene-enter', entry.isIntersecting)
  }, { threshold: .35 })
  const scan = () => document.querySelectorAll('.demo-page,.frame-cover-real').forEach(element => {
    if (!seen.has(element)) { seen.add(element); observer?.observe(element) }
  })
  scan()
  mutations = new MutationObserver(scan)
  mutations.observe(document.getElementById('app')!, {childList: true, subtree: true})
})
onBeforeUnmount(() => { observer?.disconnect(); mutations?.disconnect() })
</script>
<template><span hidden aria-hidden="true" /></template>
