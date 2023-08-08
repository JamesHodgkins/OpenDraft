#ifndef OD_SY_ALIGN
#define OD_SY_ALIGN

/**************************************************************************************
* OpenDraft:    Graphic Draw Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdAlign.h                                                             *
* Contributors: James Hodgkins                                                        *
* Date:         August 04, 2023                                                       *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A class for storing alignment states.                                             *
***************************************************************************************/


class OdAlign {
public:
	static const int LEFT = 1 << 0;
	static const int RIGHT = 1 << 1;
	static const int TOP = 1 << 2;
	static const int BOTTOM = 1 << 3;
	static const int CENTRE = 1 << 4;
	static const int MIDDLE = 1 << 5;

	OdAlign() : align_(LEFT | MIDDLE) {}
	explicit OdAlign(int align) : align_(align) {}

	void setAlign(int align) {
		align_ = align;
	}

	int getAlign() const {
		return align_;
	}

	OdAlign operator|(const OdAlign& other) const {
		return OdAlign(align_ | other.align_);
	}

	OdAlign operator|(const int rhs) {
		return OdAlign(getAlign() | rhs);
	}

private:
	int align_;
};

#endif // !OD_SY_ALIGN