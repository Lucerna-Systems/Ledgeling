import { computed } from 'vue'
import {
  tasks,
  taskError,
  conversations,
  activeConversation,
  conversationOf,
  effectiveConnection,
  connections,
  assignedPet,
  droppedFiles,
  startTasks,
  refreshTasks,
  taskCall,
} from '../assistant/state'
import type { Task } from '../assistant/types'

export function useAssistant() {
  const waitingTasksCount = computed(() => tasks.value.filter(t => t.status === 'waiting').length)

  function openConversation(taskId: number | null) {
    activeConversation.value = taskId !== null ? conversationOf(taskId) : null
  }

  function setAssignedPet(petId: number | null) {
    assignedPet.value = petId
  }

  function setDroppedFiles(files: string[]) {
    droppedFiles.value = files
  }

  return {
    tasks,
    taskError,
    conversations,
    activeConversation,
    waitingTasksCount,
    connections,
    effectiveConnection,
    openConversation,
    setAssignedPet,
    setDroppedFiles,
    startTasks,
    refreshTasks,
    taskCall,
  }
}
