#ifndef OD_SY_LINE_H
#define OD_SY_LINE_H

/**************************************************************************************
* OpenDraft:    System Line Entity Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdLine.h                                                              *
* Contributors: James Hodgkins                                                        *
* Date:         June 27, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Line Entity class for OpenDraft framework.                                        *
***************************************************************************************/


#include "System/Entities/OdEntity.h"


using namespace OD::System;

namespace OD
{
	namespace Geometry
	{

		class OdLine : public OdEntity {
		public:

			// Constructor
			OdLine(OdVector2 aStart, OdVector2 aEnd)
			{
				start = aStart;
				end = aEnd;
			}

			OdLine(int x1, int y1, int x2, int y2)
			{
				start = OdVector2(x1, y1);
				end = OdVector2(x2, y2);
			}

			OdVector2 start;
			OdVector2 end;

			// Draw
			void draw(NVGcontext* aContext) override
			{
				nvgBeginPath(aContext);
				nvgMoveTo(aContext, start.x, start.y);
				nvgLineTo(aContext, end.x, end.y);
				nvgStrokeColor(aContext, nvgRGBA(255, 0, 0, 255));
				nvgStrokeWidth(aContext, 3);
				nvgStroke(aContext);
			}

		};
	}
}


#endif // OD_SY_LINE_H