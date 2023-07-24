#ifndef OD_DAT_DBOBJECT_CPP
#define OD_DAT_DBOBJECT_CPP

/**************************************************************************************
* OpenDraft:    DB Object Class                                                       *
*-------------------------------------------------------------------------------------*
* Filename:     OdDbObject.cpp                                                        *
* Contributors: James Hodgkins                                                        *
* Date:         July 04, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   DB Object class for OpenDraft framework.                                          *
***************************************************************************************/


#include "DataManager/OdDbObject.h"


namespace OD::Data
{
	// Getters & Setters
	int OdDbObject::getHandle() const { return handle; }
	void OdDbObject::setHandle(unsigned int aHandle) { handle = aHandle; }

	int OdDbObject::getParent() const { return parent; }
	void OdDbObject::setParent(int parent) { this->parent = parent; }

} // namespace OD::Data

#endif // !OD_DAT_DBOBJECT_CPP