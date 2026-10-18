import { ref } from 'vue'
const correctionDecision = ref<'pending' | 'accepted' | 'rejected'>('pending')
const courseExcerpt = ref('')
export function useRoadshowSession() { return { correctionDecision, courseExcerpt } }
