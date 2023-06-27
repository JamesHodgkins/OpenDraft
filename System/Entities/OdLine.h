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

			OdVector2 start;
			OdVector2 end;

		};
	}
}


#endif // OD_SY_LINE_H