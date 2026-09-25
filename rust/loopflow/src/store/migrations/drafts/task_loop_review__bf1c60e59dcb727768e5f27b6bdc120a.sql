-- name: task_loop_review
-- id: bf1c60e59dcb727768e5f27b6bdc120a
-- depends_on: 

ALTER TABLE task_flow_positions ADD COLUMN review_json TEXT;
