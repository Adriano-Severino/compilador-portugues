#ifdef _WIN32
#define _CRT_SECURE_NO_WARNINGS
#endif

#include "async_runtime.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <windows.h>
typedef CRITICAL_SECTION mutex_t;
typedef CONDITION_VARIABLE condition_t;
typedef HANDLE thread_t;

static void mutex_init(mutex_t *mutex) { InitializeCriticalSection(mutex); }
static void mutex_destroy(mutex_t *mutex) { DeleteCriticalSection(mutex); }
static void mutex_lock(mutex_t *mutex) { EnterCriticalSection(mutex); }
static void mutex_unlock(mutex_t *mutex) { LeaveCriticalSection(mutex); }
static void condition_init(condition_t *condition) { InitializeConditionVariable(condition); }
static void condition_destroy(condition_t *condition) { (void)condition; }
static void condition_wait(condition_t *condition, mutex_t *mutex) {
    SleepConditionVariableCS(condition, mutex, INFINITE);
}
static void condition_signal(condition_t *condition) { WakeConditionVariable(condition); }
static void condition_broadcast(condition_t *condition) { WakeAllConditionVariable(condition); }
#else
#include <pthread.h>
typedef pthread_mutex_t mutex_t;
typedef pthread_cond_t condition_t;
typedef pthread_t thread_t;

static void mutex_init(mutex_t *mutex) { pthread_mutex_init(mutex, NULL); }
static void mutex_destroy(mutex_t *mutex) { pthread_mutex_destroy(mutex); }
static void mutex_lock(mutex_t *mutex) { pthread_mutex_lock(mutex); }
static void mutex_unlock(mutex_t *mutex) { pthread_mutex_unlock(mutex); }
static void condition_init(condition_t *condition) { pthread_cond_init(condition, NULL); }
static void condition_destroy(condition_t *condition) { pthread_cond_destroy(condition); }
static void condition_wait(condition_t *condition, mutex_t *mutex) {
    pthread_cond_wait(condition, mutex);
}
static void condition_signal(condition_t *condition) { pthread_cond_signal(condition); }
static void condition_broadcast(condition_t *condition) { pthread_cond_broadcast(condition); }
#endif

#define DEFAULT_POOL_SIZE 4
#define MAX_QUEUE_SIZE 1024

struct Task {
    TaskStatus status;
    void *result;
    int id;
    TaskFunction function;
    void *argument;
    mutex_t mutex;
    condition_t completed;
};

typedef struct ThreadPool {
    Task *queue[MAX_QUEUE_SIZE];
    size_t head;
    size_t tail;
    size_t count;
    int shutdown;
    int worker_count;
    thread_t *workers;
    mutex_t mutex;
    condition_t has_work;
    condition_t has_space;
} ThreadPool;

static ThreadPool *global_pool = NULL;
static mutex_t global_pool_mutex;
static mutex_t task_id_mutex;
static int runtime_initialized = 0;
static int current_task_id = 0;

static void runtime_init_once(void) {
    if (!runtime_initialized) {
        mutex_init(&global_pool_mutex);
        mutex_init(&task_id_mutex);
        runtime_initialized = 1;
    }
}

int next_task_id(void) {
    int id;
    runtime_init_once();
    mutex_lock(&task_id_mutex);
    id = ++current_task_id;
    mutex_unlock(&task_id_mutex);
    return id;
}

static void task_complete(Task *task, void *result, TaskStatus status) {
    mutex_lock(&task->mutex);
    task->result = result;
    task->status = status;
    condition_broadcast(&task->completed);
    mutex_unlock(&task->mutex);
}

#ifdef _WIN32
static DWORD WINAPI worker_thread(LPVOID argument)
#else
static void *worker_thread(void *argument)
#endif
{
    ThreadPool *pool = (ThreadPool *)argument;

    for (;;) {
        Task *task;
        mutex_lock(&pool->mutex);
        while (pool->count == 0 && !pool->shutdown) {
            condition_wait(&pool->has_work, &pool->mutex);
        }
        if (pool->shutdown && pool->count == 0) {
            mutex_unlock(&pool->mutex);
            break;
        }

        task = pool->queue[pool->head];
        pool->head = (pool->head + 1) % MAX_QUEUE_SIZE;
        --pool->count;
        condition_signal(&pool->has_space);
        mutex_unlock(&pool->mutex);

        mutex_lock(&task->mutex);
        task->status = TASK_RUNNING;
        mutex_unlock(&task->mutex);

        if (task->function == NULL) {
            task_complete(task, NULL, TASK_FAILED);
        } else {
            task_complete(task, task->function(task->argument), TASK_COMPLETED);
        }
    }

#ifdef _WIN32
    return 0;
#else
    return NULL;
#endif
}

void thread_pool_init(int worker_count) {
    int index;
    runtime_init_once();
    mutex_lock(&global_pool_mutex);
    if (global_pool != NULL) {
        mutex_unlock(&global_pool_mutex);
        return;
    }
    if (worker_count <= 0) {
        worker_count = DEFAULT_POOL_SIZE;
    }

    global_pool = (ThreadPool *)calloc(1, sizeof(*global_pool));
    if (global_pool == NULL) {
        mutex_unlock(&global_pool_mutex);
        return;
    }
    global_pool->worker_count = worker_count;
    global_pool->workers = (thread_t *)calloc((size_t)worker_count, sizeof(thread_t));
    if (global_pool->workers == NULL) {
        free(global_pool);
        global_pool = NULL;
        mutex_unlock(&global_pool_mutex);
        return;
    }
    mutex_init(&global_pool->mutex);
    condition_init(&global_pool->has_work);
    condition_init(&global_pool->has_space);

    for (index = 0; index < worker_count; ++index) {
#ifdef _WIN32
        global_pool->workers[index] = CreateThread(NULL, 0, worker_thread, global_pool, 0, NULL);
#else
        if (pthread_create(&global_pool->workers[index], NULL, worker_thread, global_pool) != 0) {
            global_pool->shutdown = 1;
            global_pool->worker_count = index;
            break;
        }
#endif
    }
    mutex_unlock(&global_pool_mutex);
}

void thread_pool_shutdown(void) {
    ThreadPool *pool;
    int index;
    runtime_init_once();
    mutex_lock(&global_pool_mutex);
    pool = global_pool;
    if (pool == NULL) {
        mutex_unlock(&global_pool_mutex);
        return;
    }
    global_pool = NULL;
    mutex_lock(&pool->mutex);
    pool->shutdown = 1;
    condition_broadcast(&pool->has_work);
    condition_broadcast(&pool->has_space);
    mutex_unlock(&pool->mutex);
    mutex_unlock(&global_pool_mutex);

    for (index = 0; index < pool->worker_count; ++index) {
#ifdef _WIN32
        if (pool->workers[index] != NULL) {
            WaitForSingleObject(pool->workers[index], INFINITE);
            CloseHandle(pool->workers[index]);
        }
#else
        pthread_join(pool->workers[index], NULL);
#endif
    }
    condition_destroy(&pool->has_space);
    condition_destroy(&pool->has_work);
    mutex_destroy(&pool->mutex);
    free(pool->workers);
    free(pool);
}

Task *task_create(int id) {
    Task *task = (Task *)calloc(1, sizeof(*task));
    if (task == NULL) {
        return NULL;
    }
    task->id = id;
    task->status = TASK_PENDING;
    mutex_init(&task->mutex);
    condition_init(&task->completed);
    return task;
}

void task_destroy(Task *task) {
    if (task == NULL) {
        return;
    }
    condition_destroy(&task->completed);
    mutex_destroy(&task->mutex);
    free(task);
}

void task_submit_to_pool(Task *task, TaskFunction function, void *argument) {
    ThreadPool *pool;
    if (task == NULL) {
        return;
    }
    task->function = function;
    task->argument = argument;
    thread_pool_init(DEFAULT_POOL_SIZE);

    runtime_init_once();
    mutex_lock(&global_pool_mutex);
    pool = global_pool;
    mutex_unlock(&global_pool_mutex);
    if (pool == NULL) {
        task_complete(task, NULL, TASK_FAILED);
        return;
    }

    mutex_lock(&pool->mutex);
    while (pool->count == MAX_QUEUE_SIZE && !pool->shutdown) {
        condition_wait(&pool->has_space, &pool->mutex);
    }
    if (pool->shutdown) {
        mutex_unlock(&pool->mutex);
        task_complete(task, NULL, TASK_FAILED);
        return;
    }
    pool->queue[pool->tail] = task;
    pool->tail = (pool->tail + 1) % MAX_QUEUE_SIZE;
    ++pool->count;
    condition_signal(&pool->has_work);
    mutex_unlock(&pool->mutex);
}

void task_execute_async(Task *task, TaskFunction function, void *argument) {
    task_submit_to_pool(task, function, argument);
}

void *task_await(Task *task) {
    void *result;
    if (task == NULL) {
        return NULL;
    }
    mutex_lock(&task->mutex);
    while (task->status == TASK_PENDING || task->status == TASK_RUNNING) {
        condition_wait(&task->completed, &task->mutex);
    }
    result = task->status == TASK_COMPLETED ? task->result : NULL;
    mutex_unlock(&task->mutex);
    return result;
}

void task_set_status(Task *task, int status) {
    if (task != NULL) {
        task_complete(task, task->result, (TaskStatus)status);
    }
}

void task_set_result(Task *task, void *result) {
    if (task != NULL) {
        mutex_lock(&task->mutex);
        task->result = result;
        mutex_unlock(&task->mutex);
    }
}

int task_get_status(Task *task) {
    int status;
    if (task == NULL) {
        return TASK_FAILED;
    }
    mutex_lock(&task->mutex);
    status = task->status;
    mutex_unlock(&task->mutex);
    return status;
}

void *task_get_result(Task *task) {
    void *result;
    if (task == NULL) {
        return NULL;
    }
    mutex_lock(&task->mutex);
    result = task->result;
    mutex_unlock(&task->mutex);
    return result;
}

static char *copy_string(const char *source) {
    size_t length;
    char *copy;
    if (source == NULL) {
        return NULL;
    }
    length = strlen(source) + 1;
    copy = (char *)malloc(length);
    if (copy != NULL) {
        memcpy(copy, source, length);
    }
    return copy;
}

static void *async_read_file(void *argument) {
    char *path = (char *)argument;
    FILE *file;
    long size;
    size_t read_count;
    char *contents;
    if (path == NULL) {
        return NULL;
    }
    file = fopen(path, "rb");
    free(path);
    if (file == NULL) {
        return NULL;
    }
    if (fseek(file, 0, SEEK_END) != 0 || (size = ftell(file)) < 0 || fseek(file, 0, SEEK_SET) != 0) {
        fclose(file);
        return NULL;
    }
    contents = (char *)malloc((size_t)size + 1);
    if (contents == NULL) {
        fclose(file);
        return NULL;
    }
    read_count = fread(contents, 1, (size_t)size, file);
    contents[read_count] = '\0';
    fclose(file);
    return contents;
}

typedef struct WriteArgument {
    char *path;
    char *content;
} WriteArgument;

static void *async_write_file(void *argument) {
    WriteArgument *write_argument = (WriteArgument *)argument;
    FILE *file;
    int succeeded;
    if (write_argument == NULL) {
        return NULL;
    }
    file = write_argument->path == NULL ? NULL : fopen(write_argument->path, "wb");
    succeeded = file != NULL && fputs(write_argument->content == NULL ? "" : write_argument->content, file) >= 0;
    if (file != NULL) {
        fclose(file);
    }
    free(write_argument->path);
    free(write_argument->content);
    free(write_argument);
    return succeeded ? (void *)1 : NULL;
}

static void *async_file_exists(void *argument) {
    char *path = (char *)argument;
    FILE *file;
    if (path == NULL) {
        return NULL;
    }
    file = fopen(path, "rb");
    free(path);
    if (file == NULL) {
        return NULL;
    }
    fclose(file);
    return (void *)1;
}

Task *task_create_read_file(const char *path) {
    Task *task = task_create(next_task_id());
    if (task != NULL) {
        task_submit_to_pool(task, async_read_file, copy_string(path));
    }
    return task;
}

Task *task_create_write_file(const char *path, const char *content) {
    Task *task = task_create(next_task_id());
    WriteArgument *argument;
    if (task == NULL) {
        return NULL;
    }
    argument = (WriteArgument *)malloc(sizeof(*argument));
    if (argument == NULL) {
        task_complete(task, NULL, TASK_FAILED);
        return task;
    }
    argument->path = copy_string(path);
    argument->content = copy_string(content);
    task_submit_to_pool(task, async_write_file, argument);
    return task;
}

Task *task_create_file_exists(const char *path) {
    Task *task = task_create(next_task_id());
    if (task != NULL) {
        task_submit_to_pool(task, async_file_exists, copy_string(path));
    }
    return task;
}

void free_async_result(void *result) {
    free(result);
}
