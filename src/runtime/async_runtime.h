/* Runtime nativo de async/await para o alvo LLVM. */
#ifndef POR_DO_SOL_ASYNC_RUNTIME_H
#define POR_DO_SOL_ASYNC_RUNTIME_H

#ifdef __cplusplus
extern "C" {
#endif

/* A estrutura é propositalmente opaca: o LLVM só manipula Task*. */
typedef struct Task Task;
typedef void *(*TaskFunction)(void *);

typedef enum TaskStatus {
    TASK_PENDING = 0,
    TASK_RUNNING = 1,
    TASK_COMPLETED = 2,
    TASK_FAILED = 3
} TaskStatus;

int next_task_id(void);
void thread_pool_init(int worker_count);
void thread_pool_shutdown(void);

Task *task_create(int id);
void task_destroy(Task *task);
void task_submit_to_pool(Task *task, TaskFunction function, void *argument);
void task_execute_async(Task *task, TaskFunction function, void *argument);
void *task_await(Task *task);
void task_set_status(Task *task, int status);
void task_set_result(Task *task, void *result);
int task_get_status(Task *task);
void *task_get_result(Task *task);

/* Operações nativas atualmente expostas pela geração LLVM. */
Task *task_create_read_file(const char *path);
Task *task_create_write_file(const char *path, const char *content);
Task *task_create_file_exists(const char *path);
void free_async_result(void *result);

#ifdef __cplusplus
}
#endif

#endif
