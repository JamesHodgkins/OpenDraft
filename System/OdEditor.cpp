#ifndef OD_SY_EDITOR_CPP
#define OD_SY_EDITOR_CPP

/**************************************************************************************
* OpenDraft:    System editor class                                                   *
*-------------------------------------------------------------------------------------*
* Filename:     OdEditor.cpp                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         14 July, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   An editor class to handle user input/output.                                      *
***************************************************************************************/


#include "System/OdEditor.h"


namespace OD::System
{
	OdEditor::OdEditor(OdApplication* aApplication)
	{
		application = aApplication;
	}

}

#endif // OD_SY_EDITOR_CPP