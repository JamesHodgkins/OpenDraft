#pragma once

enum class EntityType {
	Arc,
	Line,
	Circle,
	MText,
	Text
};

class Entity {
public:
	virtual EntityType GetType() const = 0;
	virtual ~Entity() = default;

private:
	unsigned int handle;
};

class Arc : public Entity {
public:
	EntityType GetType() const override {
		return EntityType::Arc;
	}
};

class Circle : public Entity {
public:
	EntityType GetType() const override {
		return EntityType::Circle;
	}
};


class Line : public Entity {
public:
	EntityType GetType() const override {
		return EntityType::Line;
	}
};

class MText : public Entity {
public:
	EntityType GetType() const override {
		return EntityType::MText;
	}
};

class Text : public Entity {
public:
	EntityType GetType() const override {
		return EntityType::Text;
	}
};